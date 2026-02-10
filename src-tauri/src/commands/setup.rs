use crate::backends::BackendKind;
use crate::storage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::process::Command;
use tokio::time::timeout;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

const CLI_CACHE_TTL_SECONDS: u64 = 604800; // 7 days
const CLI_SHELL_DETECTION_TIMEOUT: Duration = Duration::from_secs(2);
const CLI_VERSION_TIMEOUT: Duration = Duration::from_secs(3);
const CLI_AUTH_PROBE_TIMEOUT: Duration = Duration::from_secs(15);

// ============================================================================
// Platform-specific command creation (hidden window on Windows)
// ============================================================================

/// Create a command that won't spawn a visible console window on Windows
#[cfg(target_os = "windows")]
fn create_hidden_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Create a command (passthrough on non-Windows platforms)
#[cfg(not(target_os = "windows"))]
fn create_hidden_command(program: &str) -> Command {
    Command::new(program)
}

// ============================================================================
// Windows CLI path normalization
// ============================================================================

/// Normalize Windows CLI path to prefer .cmd extension for npm-installed binaries.
/// This fixes error 193 when the shell script (without extension) is found before claude.cmd.
#[cfg(target_os = "windows")]
fn normalize_windows_cli_path(path: &str) -> String {
    let path_obj = Path::new(path);
    // If already .cmd or .exe, return as-is
    if let Some(ext) = path_obj.extension() {
        let ext_lower = ext.to_string_lossy().to_lowercase();
        if ext_lower == "cmd" || ext_lower == "exe" {
            return path.to_string();
        }
    }
    // For npm paths, try .cmd version
    let cmd_path = format!("{}.cmd", path);
    if Path::new(&cmd_path).is_file() {
        return cmd_path;
    }
    path.to_string()
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
fn normalize_windows_cli_path(path: &str) -> String {
    path.to_string()
}

// ============================================================================
// Platform-specific CLI path detection
// ============================================================================

/// Get common installation paths for Claude CLI on macOS
#[cfg(target_os = "macos")]
fn get_common_claude_paths(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".local/bin/claude"),
        home.join(".claude/local/bin/claude"),
        PathBuf::from("/usr/local/bin/claude"),
        PathBuf::from("/opt/homebrew/bin/claude"),
    ]
}

/// Get common installation paths for Claude CLI on Linux
#[cfg(target_os = "linux")]
fn get_common_claude_paths(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".local/bin/claude"),
        home.join(".claude/local/bin/claude"),
        PathBuf::from("/usr/local/bin/claude"),
    ]
}

/// Get common installation paths for Claude CLI on Windows
#[cfg(target_os = "windows")]
fn get_common_claude_paths(home: &Path) -> Vec<PathBuf> {
    let mut paths = vec![
        home.join(".claude\\local\\claude.exe"),
        home.join(".local\\bin\\claude.exe"),
    ];

    // Add %LOCALAPPDATA%\Claude paths
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let local_path = PathBuf::from(local_app_data);
        paths.push(local_path.join("Claude\\claude.exe"));
        paths.push(local_path.join("Programs\\Claude\\claude.exe"));
    }

    // Add %PROGRAMFILES% paths
    if let Some(program_files) = std::env::var_os("PROGRAMFILES") {
        paths.push(PathBuf::from(program_files).join("Claude\\claude.exe"));
    }

    // Add %APPDATA%\npm for global npm installs
    if let Some(app_data) = std::env::var_os("APPDATA") {
        paths.push(PathBuf::from(app_data).join("npm\\claude.cmd"));
    }

    paths
}

#[cfg(target_os = "macos")]
fn get_common_codex_paths(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".local/bin/codex"),
        PathBuf::from("/usr/local/bin/codex"),
        PathBuf::from("/opt/homebrew/bin/codex"),
    ]
}

#[cfg(target_os = "linux")]
fn get_common_codex_paths(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".local/bin/codex"),
        PathBuf::from("/usr/local/bin/codex"),
    ]
}

#[cfg(target_os = "windows")]
fn get_common_codex_paths(home: &Path) -> Vec<PathBuf> {
    let mut paths = vec![home.join(".local\\bin\\codex.exe")];
    if let Some(app_data) = std::env::var_os("APPDATA") {
        paths.push(PathBuf::from(app_data).join("npm\\codex.cmd"));
    }
    paths
}

fn get_common_cli_paths_for_backend(backend: &str, home: &Path) -> Vec<PathBuf> {
    match backend {
        "codex" => get_common_codex_paths(home),
        _ => get_common_claude_paths(home),
    }
}

// ============================================================================
// Platform-specific shell-based CLI detection
// ============================================================================

/// Try to find claude using shell commands on Unix (macOS/Linux)
#[cfg(any(target_os = "macos", target_os = "linux"))]
async fn try_shell_based_cli_detection(binary: &str) -> Option<String> {
    // Determine user's shell
    let user_shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let is_zsh = user_shell.contains("zsh");

    // Try sourcing interactive shell config (where PATH is usually set)
    let source_cmd = if is_zsh {
        format!("source ~/.zshrc 2>/dev/null; which {}", binary)
    } else {
        format!("source ~/.bashrc 2>/dev/null; which {}", binary)
    };

    if let Some(cli_path) = try_shell_which(&user_shell, &["-c", source_cmd.as_str()]).await {
        return Some(cli_path);
    }

    // Try login shell with user's preferred shell
    let which_cmd = format!("which {}", binary);
    if let Some(cli_path) = try_shell_which(&user_shell, &["-l", "-c", which_cmd.as_str()]).await {
        return Some(cli_path);
    }

    // Final fallback: try both common shells with login flag
    let which_cmd = format!("which {}", binary);
    for shell in ["/bin/zsh", "/bin/bash"] {
        if let Some(cli_path) = try_shell_which(shell, &["-l", "-c", which_cmd.as_str()]).await {
            return Some(cli_path);
        }
    }

    None
}

/// Try to find claude using where command on Windows
#[cfg(target_os = "windows")]
async fn try_shell_based_cli_detection(binary: &str) -> Option<String> {
    // On Windows, PATH is global, so we can use 'where' command directly
    // Try cmd.exe first
    let where_cmd = format!("where {}", binary);
    if let Some(cli_path) = try_shell_which("cmd.exe", &["/c", where_cmd.as_str()]).await {
        // 'where' may return multiple lines, take the first one
        let first_path = cli_path.lines().next()?.to_string();
        // Normalize to .cmd if it exists (fixes error 193 with npm installs)
        return Some(normalize_windows_cli_path(&first_path));
    }

    // Try PowerShell
    let ps_cmd = format!(
        "Get-Command {} -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source",
        binary
    );
    if let Some(cli_path) = try_shell_which("powershell.exe", &["-Command", ps_cmd.as_str()]).await
    {
        // Normalize to .cmd if it exists (fixes error 193 with npm installs)
        return Some(normalize_windows_cli_path(&cli_path));
    }

    None
}

// ============================================================================
// Platform-specific OAuth token detection
// ============================================================================

/// Check if OAuth token exists in Claude Desktop's config (macOS)
#[cfg(target_os = "macos")]
fn get_oauth_config_path(home_dir: &Path) -> PathBuf {
    home_dir.join("Library/Application Support/Claude/config.json")
}

/// Check if OAuth token exists in Claude Desktop's config (Windows)
#[cfg(target_os = "windows")]
fn get_oauth_config_path(home_dir: &Path) -> PathBuf {
    // Always derive from home_dir for testability
    // This produces the same path as %APPDATA%\Claude\config.json when home_dir is the user's home
    home_dir
        .join("AppData")
        .join("Roaming")
        .join("Claude")
        .join("config.json")
}

/// Check if OAuth token exists in Claude Desktop's config (Linux)
#[cfg(target_os = "linux")]
fn get_oauth_config_path(home_dir: &Path) -> PathBuf {
    home_dir.join(".config/Claude/config.json")
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CliStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub authenticated: bool,
    pub path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BackendCliStatus {
    pub backend: String,
    pub installed: bool,
    pub authenticated: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub install_hint: Option<String>,
    pub auth_hint: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliInstallStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CliAuthStatus {
    pub authenticated: bool,
}

fn install_hint_for_backend(backend: &str) -> Option<String> {
    match backend {
        "claude" | "claudecli" => {
            #[cfg(target_os = "windows")]
            {
                Some("irm https://claude.ai/install.ps1 | iex".to_string())
            }
            #[cfg(not(target_os = "windows"))]
            {
                Some("curl -fsSL https://claude.ai/install.sh | bash".to_string())
            }
        }
        "codex" => Some("npm install -g @openai/codex".to_string()),
        _ => None,
    }
}

fn auth_hint_for_backend(backend: &str) -> Option<String> {
    match backend {
        "claude" | "claudecli" => Some("Run `claude` and complete login".to_string()),
        "codex" => Some("Run `codex login` and complete login".to_string()),
        _ => None,
    }
}

fn validate_backend_option(backend: Option<String>) -> Result<Option<String>, String> {
    match backend {
        Some(name) => {
            let kind = name
                .parse::<BackendKind>()
                .map_err(|e| format!("Invalid backend '{}': {}", name, e))?;
            Ok(Some(kind.to_string()))
        }
        None => Ok(None),
    }
}

/// Try to find claude by running a command in a shell
async fn run_hidden_command_with_timeout(
    program: &str,
    args: &[&str],
    timeout_duration: Duration,
) -> Option<std::process::Output> {
    timeout(
        timeout_duration,
        create_hidden_command(program).args(args).output(),
    )
    .await
    .ok()?
    .ok()
}

/// Try to find claude by running a command in a shell
async fn try_shell_which(shell: &str, args: &[&str]) -> Option<String> {
    run_hidden_command_with_timeout(shell, args, CLI_SHELL_DETECTION_TIMEOUT)
        .await
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Get CLI version by running the binary directly.
async fn get_cli_version(cli_path: &str) -> Option<String> {
    run_hidden_command_with_timeout(cli_path, &["--version"], CLI_VERSION_TIMEOUT)
        .await
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

async fn detect_cli_install_at_path(cli_path: &str) -> Option<CliInstallStatus> {
    let normalized = normalize_windows_cli_path(cli_path);

    if let Some(version) = get_cli_version(&normalized).await {
        return Some(CliInstallStatus {
            installed: true,
            version: Some(version),
            path: Some(normalized),
        });
    }

    let responds_to_help =
        run_hidden_command_with_timeout(&normalized, &["--help"], CLI_VERSION_TIMEOUT)
            .await
            .map(|out| out.status.success())
            .unwrap_or(false);
    if responds_to_help {
        return Some(CliInstallStatus {
            installed: true,
            version: None,
            path: Some(normalized),
        });
    }

    None
}

pub async fn check_backend_cli_installed_internal(backend: &str) -> CliInstallStatus {
    let backend_name = match backend.parse::<BackendKind>() {
        Ok(kind) => kind.to_string(),
        Err(_) => {
            return CliInstallStatus {
                installed: false,
                version: None,
                path: None,
            }
        }
    };

    let binary = if backend_name == "codex" {
        "codex"
    } else {
        "claude"
    };

    // Strategy:
    // 1. Check custom configured path (if set)
    // 2. Check common installation paths directly
    // 3. Try platform-specific shell/command detection

    if let Ok(Some(custom_path)) = storage::get_backend_cli_path(&backend_name) {
        if let Some(status) = detect_cli_install_at_path(&custom_path).await {
            return status;
        }
    }

    // Check common installation paths first
    if let Some(home) = dirs::home_dir() {
        let common_paths = get_common_cli_paths_for_backend(&backend_name, &home);

        for path in common_paths {
            if path.is_file() {
                let path_str = path.to_string_lossy().to_string();
                if let Some(status) = detect_cli_install_at_path(&path_str).await {
                    return status;
                }
            }
        }
    }

    // Try platform-specific shell-based detection
    if let Some(detected_path) = try_shell_based_cli_detection(binary).await {
        if let Some(status) = detect_cli_install_at_path(&detected_path).await {
            return status;
        }
        return CliInstallStatus {
            installed: true,
            version: None,
            path: Some(detected_path),
        };
    }

    CliInstallStatus {
        installed: false,
        version: None,
        path: None,
    }
}

/// Internal function for checking if Claude CLI is installed.
/// Used by both the Tauri command and the Claude backend adapter.
pub async fn check_cli_installed_internal() -> CliInstallStatus {
    check_backend_cli_installed_internal("claude").await
}

#[tauri::command]
pub async fn check_cli_installed() -> Result<CliInstallStatus, String> {
    Ok(check_cli_installed_internal().await)
}

/// Check if OAuth token exists in Claude Desktop's config
fn check_oauth_token(home_dir: &Path) -> bool {
    let config_path = get_oauth_config_path(home_dir);

    if let Ok(content) = std::fs::read_to_string(&config_path) {
        // Check if oauth:tokenCache field exists and has a value
        return content.contains("\"oauth:tokenCache\"");
    }
    false
}

/// Check legacy credentials file location
fn check_legacy_credentials(home_dir: &Path) -> bool {
    let creds_path = home_dir.join(".claude").join(".credentials.json");
    creds_path.exists()
}

/// Test authentication by running a simple claude prompt
async fn test_claude_auth(claude_path: &str) -> bool {
    // Run claude with a simple "hi" prompt using --print mode and haiku (fastest, no thinking)
    // If authenticated, it will respond. If not, it will fail.
    match run_hidden_command_with_timeout(
        claude_path,
        &["-p", "hi", "--model", "haiku"],
        CLI_AUTH_PROBE_TIMEOUT,
    )
    .await
    {
        Some(output) => {
            // If exit code is 0 and we got some output, auth is working
            output.status.success() && !output.stdout.is_empty()
        }
        None => false,
    }
}

/// Test authentication by running a simple codex prompt.
async fn test_codex_auth(codex_path: &str) -> bool {
    match run_hidden_command_with_timeout(
        codex_path,
        &[
            "--sandbox",
            "read-only",
            "exec",
            "--json",
            "--skip-git-repo-check",
            "Reply with exactly: OK",
        ],
        CLI_AUTH_PROBE_TIMEOUT,
    )
    .await
    {
        Some(output) => output.status.success(),
        None => false,
    }
}

/// Internal function for checking if Claude CLI is authenticated.
/// Used by both the Tauri command and the Claude backend adapter.
pub async fn check_cli_authenticated_internal() -> CliAuthStatus {
    // Check environment variable first (for API key users)
    if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        return CliAuthStatus {
            authenticated: true,
        };
    }

    if let Some(home) = dirs::home_dir() {
        // Check Claude Desktop's OAuth token (most common for Pro/Max users)
        if check_oauth_token(&home) {
            return CliAuthStatus {
                authenticated: true,
            };
        }

        // Check legacy credentials file location
        if check_legacy_credentials(&home) {
            return CliAuthStatus {
                authenticated: true,
            };
        }
    }

    // Fall back to actually testing claude with a simple prompt
    // This handles cases where auth is stored in unexpected locations
    let install_status = check_cli_installed_internal().await;
    if let Some(claude_path) = install_status.path {
        if test_claude_auth(&claude_path).await {
            return CliAuthStatus {
                authenticated: true,
            };
        }
    }

    CliAuthStatus {
        authenticated: false,
    }
}

pub async fn check_backend_cli_authenticated_internal(backend: &str) -> CliAuthStatus {
    let backend_name = match backend.parse::<BackendKind>() {
        Ok(kind) => kind.to_string(),
        Err(_) => {
            return CliAuthStatus {
                authenticated: false,
            }
        }
    };

    if backend_name == "codex" {
        let install_status = check_backend_cli_installed_internal("codex").await;
        if let Some(path) = install_status.path {
            if test_codex_auth(&path).await {
                return CliAuthStatus {
                    authenticated: true,
                };
            }
        }
        return CliAuthStatus {
            authenticated: false,
        };
    }

    check_cli_authenticated_internal().await
}

#[tauri::command]
pub async fn check_cli_authenticated() -> Result<CliAuthStatus, String> {
    Ok(check_cli_authenticated_internal().await)
}

#[tauri::command]
pub async fn check_cli_status() -> Result<CliStatus, String> {
    let install_status = check_cli_installed().await?;

    if !install_status.installed {
        return Ok(CliStatus {
            installed: false,
            version: None,
            authenticated: false,
            path: None,
        });
    }

    let auth_status = check_cli_authenticated().await?;

    Ok(CliStatus {
        installed: install_status.installed,
        version: install_status.version,
        authenticated: auth_status.authenticated,
        path: install_status.path,
    })
}

async fn check_backend_cli_status_internal(backend: &str) -> CliStatus {
    let install = check_backend_cli_installed_internal(backend).await;
    if !install.installed {
        return CliStatus {
            installed: false,
            version: None,
            authenticated: false,
            path: None,
        };
    }

    let auth = check_backend_cli_authenticated_internal(backend).await;
    CliStatus {
        installed: install.installed,
        version: install.version,
        authenticated: auth.authenticated,
        path: install.path,
    }
}

#[tauri::command]
pub async fn check_backend_cli_installed(backend: String) -> Result<CliInstallStatus, String> {
    Ok(check_backend_cli_installed_internal(&backend).await)
}

#[tauri::command]
pub async fn check_backend_cli_authenticated(backend: String) -> Result<CliAuthStatus, String> {
    Ok(check_backend_cli_authenticated_internal(&backend).await)
}

#[tauri::command]
pub async fn check_all_backends_status() -> Result<Vec<BackendCliStatus>, String> {
    let backends = [BackendKind::Claude, BackendKind::Codex];

    let futures: Vec<_> = backends
        .iter()
        .map(|backend| {
            let backend_name = backend.to_string();
            async move {
                let install = check_backend_cli_installed_internal(&backend_name).await;
                let auth = if install.installed {
                    check_backend_cli_authenticated_internal(&backend_name).await
                } else {
                    CliAuthStatus {
                        authenticated: false,
                    }
                };
                BackendCliStatus {
                    backend: backend_name.clone(),
                    installed: install.installed,
                    authenticated: auth.authenticated,
                    version: install.version,
                    path: install.path,
                    install_hint: install_hint_for_backend(&backend_name),
                    auth_hint: auth_hint_for_backend(&backend_name),
                }
            }
        })
        .collect();

    Ok(futures::future::join_all(futures).await)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupInfo {
    pub onboarding_completed: bool,
    pub cli_status: Option<CliStatus>,
    pub cli_status_fresh: bool,
    pub default_folder: Option<String>,
    pub cli_path: Option<String>,
    pub default_backend: String,
    #[serde(default)]
    pub backend_cli_paths: HashMap<String, String>,
}

#[tauri::command]
pub async fn get_startup_info() -> Result<StartupInfo, String> {
    let onboarding_completed = storage::get_onboarding_completed().map_err(|e| e.to_string())?;
    let default_folder = storage::get_default_folder().map_err(|e| e.to_string())?;
    let cli_path = storage::get_cli_path().map_err(|e| e.to_string())?;
    let default_backend = storage::get_default_backend()
        .map_err(|e| e.to_string())?
        .and_then(|stored| {
            stored
                .parse::<BackendKind>()
                .ok()
                .map(|kind| kind.to_string())
        })
        .unwrap_or_else(|| "claude".to_string());
    let backend_cli_paths = storage::get_backend_cli_paths().map_err(|e| e.to_string())?;

    let cache = storage::get_cli_status_cache().map_err(|e| e.to_string())?;
    let (cli_status, cli_status_fresh) = match cache {
        Some(cached) => {
            // Only surface cached status when it matches the default backend.
            // Legacy caches without backend are treated as "claude".
            let cached_backend = cached.backend.as_deref().unwrap_or("claude");
            if cached_backend != default_backend {
                (None, false)
            } else {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or(Duration::ZERO)
                    .as_secs();
                let age = now.saturating_sub(cached.cached_at);
                let fresh = age < CLI_CACHE_TTL_SECONDS;
                (Some(cached.status), fresh)
            }
        }
        None => (None, false),
    };

    Ok(StartupInfo {
        onboarding_completed,
        cli_status,
        cli_status_fresh,
        default_folder,
        cli_path,
        default_backend,
        backend_cli_paths,
    })
}

#[tauri::command]
pub async fn complete_onboarding(
    default_folder: Option<String>,
    default_backend: Option<String>,
) -> Result<(), String> {
    let default_backend = validate_backend_option(default_backend)?;

    // Cache the selected backend's status.
    let backend_name = default_backend
        .clone()
        .unwrap_or_else(|| "claude".to_string());
    let status = check_backend_cli_status_internal(&backend_name).await;
    storage::set_cli_status_cache(status, Some(backend_name))
        .map_err(|e| e.to_string())?;

    // Save the default folder
    storage::set_default_folder(default_folder).map_err(|e| e.to_string())?;
    storage::set_default_backend(default_backend).map_err(|e| e.to_string())?;

    // Mark onboarding as completed
    storage::set_onboarding_completed(true).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn get_default_backend() -> Result<Option<String>, String> {
    let backend = storage::get_default_backend().map_err(|e| e.to_string())?;
    Ok(backend.and_then(|name| {
        name.parse::<BackendKind>()
            .ok()
            .map(|kind| kind.to_string())
    }))
}

#[tauri::command]
pub async fn set_default_backend(backend: Option<String>) -> Result<(), String> {
    let backend = validate_backend_option(backend)?;
    storage::set_default_backend(backend).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn set_default_folder(path: Option<String>) -> Result<(), String> {
    storage::set_default_folder(path).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn reset_onboarding() -> Result<(), String> {
    storage::set_onboarding_completed(false).map_err(|e| e.to_string())?;
    storage::clear_cli_status_cache().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_cli_path() -> Result<Option<String>, String> {
    storage::get_cli_path().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_cli_path(path: Option<String>) -> Result<(), String> {
    storage::set_cli_path(path).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_backend_cli_path(backend: String) -> Result<Option<String>, String> {
    storage::get_backend_cli_path(&backend).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_backend_cli_path(backend: String, path: Option<String>) -> Result<(), String> {
    storage::set_backend_cli_path(&backend, path).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_check_legacy_credentials_exists() {
        let temp_dir = TempDir::new().unwrap();
        let claude_dir = temp_dir.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(claude_dir.join(".credentials.json"), "{}").unwrap();

        assert!(check_legacy_credentials(temp_dir.path()));
    }

    #[test]
    fn test_check_legacy_credentials_not_exists() {
        let temp_dir = TempDir::new().unwrap();
        // No .claude directory or credentials file

        assert!(!check_legacy_credentials(temp_dir.path()));
    }

    #[test]
    fn test_check_oauth_token_exists() {
        let temp_dir = TempDir::new().unwrap();
        // Use the platform-specific path function to create the config in the right location
        let config_path = get_oauth_config_path(temp_dir.path());
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        std::fs::write(&config_path, r#"{"oauth:tokenCache": "sometoken"}"#).unwrap();

        assert!(check_oauth_token(temp_dir.path()));
    }

    #[test]
    fn test_check_oauth_token_not_exists() {
        let temp_dir = TempDir::new().unwrap();
        // No config file

        assert!(!check_oauth_token(temp_dir.path()));
    }

    #[test]
    fn test_check_oauth_token_no_token_field() {
        let temp_dir = TempDir::new().unwrap();
        // Use the platform-specific path function to create the config in the right location
        let config_path = get_oauth_config_path(temp_dir.path());
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        std::fs::write(&config_path, r#"{"darkMode": "light"}"#).unwrap();

        assert!(!check_oauth_token(temp_dir.path()));
    }
}
