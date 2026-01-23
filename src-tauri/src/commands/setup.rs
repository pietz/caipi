use crate::storage;
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const CLI_CACHE_TTL_SECONDS: u64 = 604800; // 7 days

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CliStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub authenticated: bool,
    pub path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CliInstallStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CliAuthStatus {
    pub authenticated: bool,
}

#[tauri::command]
pub async fn check_cli_installed() -> Result<CliInstallStatus, String> {
    // Use a login shell to get the user's full PATH
    // This works in production app bundles where PATH is limited
    let shell_result = Command::new("/bin/zsh")
        .args(["-l", "-c", "which claude"])
        .output();

    let path = match shell_result {
        Ok(output) if output.status.success() => {
            Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
        }
        _ => None,
    };

    let Some(claude_path) = path else {
        return Ok(CliInstallStatus {
            installed: false,
            version: None,
            path: None,
        });
    };

    // Check version using a login shell as well
    let version = Command::new("/bin/zsh")
        .args(["-l", "-c", "claude --version"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    Ok(CliInstallStatus {
        installed: true,
        version,
        path: Some(claude_path),
    })
}

#[tauri::command]
pub async fn check_cli_authenticated() -> Result<CliAuthStatus, String> {
    // Skip the slow auth check - we'll handle auth errors at chat time
    // The CLI will provide clear error messages if not authenticated
    Ok(CliAuthStatus { authenticated: true })
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

#[derive(Debug, Serialize, Deserialize)]
pub struct StartupInfo {
    pub onboarding_completed: bool,
    pub cli_status: Option<CliStatus>,
    pub cli_status_fresh: bool,
    pub default_folder: Option<String>,
}

#[tauri::command]
pub async fn get_startup_info() -> Result<StartupInfo, String> {
    let onboarding_completed = storage::get_onboarding_completed().map_err(|e| e.to_string())?;
    let default_folder = storage::get_default_folder().map_err(|e| e.to_string())?;

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
