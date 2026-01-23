mod commands;
mod claude;
mod storage;

use commands::chat::SessionStore;
use claude::agent::{PermissionChannels, PlanChannels};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let session_store: SessionStore = Arc::new(Mutex::new(HashMap::new()));
    let permission_channels: PermissionChannels = Arc::new(Mutex::new(HashMap::new()));
    let plan_channels: PlanChannels = Arc::new(Mutex::new(HashMap::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(session_store)
        .manage(permission_channels)
        .manage(plan_channels)
        .invoke_handler(tauri::generate_handler![
            // Setup commands
            commands::check_cli_status,
            commands::check_cli_installed,
            commands::check_cli_authenticated,
            commands::get_startup_info,
            commands::complete_onboarding,
            commands::reset_onboarding,
            // Folder commands
            commands::validate_folder,
            commands::get_recent_folders,
            commands::save_recent_folder,
            // File commands
            commands::list_directory,
            // Chat commands
            commands::create_session,
            commands::send_message,
            commands::respond_permission,
            commands::respond_plan,
            commands::get_session_messages,
            commands::abort_session,
            commands::set_permission_mode,
            commands::set_model,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
