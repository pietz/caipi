use crate::backends::BackendRegistry;
use crate::storage;
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

const CLI_CACHE_TTL_SECONDS: u64 = 604800; // 7 days

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
pub struct CliInstallStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CliAuthStatus {
    pub authenticated: bool,
}

/// Try to find claude by running a command in a shell
fn try_shell_which(shell: &str, args: &[&str]) -> Option<String> {
    Command::new(shell)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Get claude version by running the binary directly
fn get_claude_version(claude_path: &str) -> Option<String> {
    Command::new(claude_path)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Internal implementation of check_cli_installed for use by backend adapters.
pub async fn check_cli_installed_internal() -> CliInstallStatus {
    // Strategy:
    // 1. Check common installation paths directly (fastest, most reliable)
    // 2. Try user's shell with interactive config sourced (.zshrc/.bashrc)
    // 3. Fall back to login shell attempts

    // Check common installation paths first
    if let Some(home) = dirs::home_dir() {
        let common_paths = [
            home.join(".local/bin/claude"),
            home.join(".claude/local/bin/claude"),
            std::path::PathBuf::from("/usr/local/bin/claude"),
            std::path::PathBuf::from("/opt/homebrew/bin/claude"),
        ];

        for path in common_paths {
            if path.is_file() {
                let path_str = path.to_string_lossy().to_string();
                if let Some(version) = get_claude_version(&path_str) {
                    return CliInstallStatus {
                        installed: true,
                        version: Some(version),
                        path: Some(path_str),
                    };
                }
            }
        }
    }

    // Determine user's shell
    let user_shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let is_zsh = user_shell.contains("zsh");

    // Try sourcing interactive shell config (where PATH is usually set)
    let source_cmd = if is_zsh {
        "source ~/.zshrc 2>/dev/null; which claude"
    } else {
        "source ~/.bashrc 2>/dev/null; which claude"
    };

    if let Some(claude_path) = try_shell_which(&user_shell, &["-c", source_cmd]) {
        let version = get_claude_version(&claude_path);
        return CliInstallStatus {
            installed: true,
            version,
            path: Some(claude_path),
        };
    }

    // Try login shell with user's preferred shell
    if let Some(claude_path) = try_shell_which(&user_shell, &["-l", "-c", "which claude"]) {
        let version = get_claude_version(&claude_path);
        return CliInstallStatus {
            installed: true,
            version,
            path: Some(claude_path),
        };
    }

    // Final fallback: try both common shells with login flag
    for shell in ["/bin/zsh", "/bin/bash"] {
        if let Some(claude_path) = try_shell_which(shell, &["-l", "-c", "which claude"]) {
            let version = get_claude_version(&claude_path);
            return CliInstallStatus {
                installed: true,
                version,
                path: Some(claude_path),
            };
        }
    }

    CliInstallStatus {
        installed: false,
        version: None,
        path: None,
    }
}

#[tauri::command]
pub async fn check_cli_installed() -> Result<CliInstallStatus, String> {
    Ok(check_cli_installed_internal().await)
}

/// Check if OAuth token exists in Claude Desktop's config
fn check_oauth_token(home_dir: &std::path::Path) -> bool {
    let config_path = home_dir
        .join("Library/Application Support/Claude/config.json");

    if let Ok(content) = std::fs::read_to_string(&config_path) {
        // Check if oauth:tokenCache field exists and has a value
        return content.contains("\"oauth:tokenCache\"");
    }
    false
}

/// Check legacy credentials file location
fn check_legacy_credentials(home_dir: &std::path::Path) -> bool {
    let creds_path = home_dir.join(".claude").join(".credentials.json");
    creds_path.exists()
}

/// Test authentication by running a simple claude prompt
fn test_claude_auth(claude_path: &str) -> bool {
    // Run claude with a simple "hi" prompt using --print mode and haiku (fastest, no thinking)
    // If authenticated, it will respond. If not, it will fail.
    match Command::new(claude_path)
        .args(["-p", "hi", "--model", "haiku"])
        .output()
    {
        Ok(output) => {
            // If exit code is 0 and we got some output, auth is working
            output.status.success() && !output.stdout.is_empty()
        }
        Err(_) => false,
    }
}

/// Internal implementation of check_cli_authenticated for use by backend adapters.
pub async fn check_cli_authenticated_internal() -> CliAuthStatus {
    // Check environment variable first (for API key users)
    if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        return CliAuthStatus { authenticated: true };
    }

    if let Some(home) = dirs::home_dir() {
        // Check Claude Desktop's OAuth token (most common for Pro/Max users)
        if check_oauth_token(&home) {
            return CliAuthStatus { authenticated: true };
        }

        // Check legacy credentials file location
        if check_legacy_credentials(&home) {
            return CliAuthStatus { authenticated: true };
        }
    }

    // Fall back to actually testing claude with a simple prompt
    // This handles cases where auth is stored in unexpected locations
    let install_status = check_cli_installed_internal().await;
    if let Some(claude_path) = install_status.path {
        if test_claude_auth(&claude_path) {
            return CliAuthStatus { authenticated: true };
        }
    }

    CliAuthStatus { authenticated: false }
}

#[tauri::command]
pub async fn check_cli_authenticated() -> Result<CliAuthStatus, String> {
    Ok(check_cli_authenticated_internal().await)
}

#[tauri::command]
pub async fn check_cli_status() -> Result<CliStatus, String> {
    let install_status = check_cli_installed_internal().await;

    if !install_status.installed {
        return Ok(CliStatus {
            installed: false,
            version: None,
            authenticated: false,
            path: None,
        });
    }

    let auth_status = check_cli_authenticated_internal().await;

    Ok(CliStatus {
        installed: install_status.installed,
        version: install_status.version,
        authenticated: auth_status.authenticated,
        path: install_status.path,
    })
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupInfo {
    pub onboarding_completed: bool,
    pub cli_status: Option<CliStatus>,
    pub cli_status_fresh: bool,
    pub default_folder: Option<String>,
    pub cli_path: Option<String>,
}

#[tauri::command]
pub async fn get_startup_info() -> Result<StartupInfo, String> {
    let onboarding_completed = storage::get_onboarding_completed().map_err(|e| e.to_string())?;
    let default_folder = storage::get_default_folder().map_err(|e| e.to_string())?;
    let cli_path = storage::get_cli_path().map_err(|e| e.to_string())?;

    let cache = storage::get_cli_status_cache().map_err(|e| e.to_string())?;

    let (cli_status, cli_status_fresh) = match cache {
        Some(cached) => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let age = now.saturating_sub(cached.cached_at);
            let fresh = age < CLI_CACHE_TTL_SECONDS;
            (Some(cached.status), fresh)
        }
        None => (None, false),
    };

    Ok(StartupInfo {
        onboarding_completed,
        cli_status,
        cli_status_fresh,
        default_folder,
        cli_path,
    })
}

#[tauri::command]
pub async fn complete_onboarding(default_folder: Option<String>) -> Result<(), String> {
    // Get fresh CLI status and cache it
    let status = check_cli_status().await?;
    storage::set_cli_status_cache(status).map_err(|e| e.to_string())?;

    // Save the default folder
    storage::set_default_folder(default_folder).map_err(|e| e.to_string())?;

    // Mark onboarding as completed
    storage::set_onboarding_completed(true).map_err(|e| e.to_string())?;

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

/// Status for a single backend
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BackendStatus {
    pub kind: String,
    pub installed: bool,
    pub version: Option<String>,
    pub authenticated: bool,
}

/// Check installation and authentication status for all registered backends
#[tauri::command]
pub async fn check_backends_status(app: AppHandle) -> Result<Vec<BackendStatus>, String> {
    let registry: tauri::State<'_, Arc<BackendRegistry>> = app.state();

    let mut statuses = Vec::new();

    for kind in registry.available_backends() {
        if let Some(backend) = registry.get(kind) {
            let install_status = backend.check_installed().await.map_err(|e| e.to_string())?;
            let auth_status = backend.check_authenticated().await.map_err(|e| e.to_string())?;

            statuses.push(BackendStatus {
                kind: kind.to_string(),
                installed: install_status.installed,
                version: install_status.version,
                authenticated: auth_status.authenticated,
            });
        }
    }

    Ok(statuses)
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
        let config_dir = temp_dir.path().join("Library/Application Support/Claude");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("config.json"),
            r#"{"oauth:tokenCache": "sometoken"}"#,
        )
        .unwrap();

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
        let config_dir = temp_dir.path().join("Library/Application Support/Claude");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("config.json"), r#"{"darkMode": "light"}"#).unwrap();

        assert!(!check_oauth_token(temp_dir.path()));
    }
}
