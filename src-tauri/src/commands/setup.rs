use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Serialize, Deserialize)]
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
    // Check if claude is installed
    let which_result = Command::new("which").arg("claude").output();

    let (installed, path) = match which_result {
        Ok(output) if output.status.success() => {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            (true, Some(path))
        }
        _ => (false, None),
    };

    if !installed {
        return Ok(CliInstallStatus {
            installed: false,
            version: None,
            path: None,
        });
    }

    // Check version
    let version = Command::new("claude")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    Ok(CliInstallStatus {
        installed,
        version,
        path,
    })
}

#[tauri::command]
pub async fn check_cli_authenticated() -> Result<CliAuthStatus, String> {
    // Check authentication by attempting a simple operation
    // Use --print to avoid any interactive prompts
    let auth_check = Command::new("claude")
        .args(["--print", "--output-format", "json", "echo test"])
        .output();

    let authenticated = auth_check
        .map(|o| o.status.success())
        .unwrap_or(false);

    Ok(CliAuthStatus { authenticated })
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
