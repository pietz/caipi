mod commands;
mod claude;
mod storage;

use commands::chat::SessionStore;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let session_store: SessionStore = Arc::new(Mutex::new(HashMap::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(session_store)
        .invoke_handler(tauri::generate_handler![
            // Setup commands
            commands::check_cli_status,
            commands::check_cli_installed,
            commands::check_cli_authenticated,
            // Folder commands
            commands::validate_folder,
            commands::get_recent_folders,
            commands::save_recent_folder,
            // Chat commands
            commands::create_session,
            commands::send_message,
            commands::respond_permission,
            commands::get_session_messages,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
