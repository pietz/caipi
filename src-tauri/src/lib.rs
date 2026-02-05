mod backends;
mod commands;
mod claude;
mod storage;

use backends::{BackendKind, BackendRegistry, BackendSession};
use backends::claude::{ClaudeBackend, ClaudeCliBackend};
use commands::chat::SessionStore;
use claude::agent::PermissionChannels;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let session_store: SessionStore = Arc::new(Mutex::new(HashMap::new()));
    let permission_channels: PermissionChannels = Arc::new(Mutex::new(HashMap::new()));

    // Initialize backend registry with Claude backends
    let mut registry = BackendRegistry::new();
    registry.register(Arc::new(ClaudeBackend::new()));
    registry.register(Arc::new(ClaudeCliBackend::new()));

    // Allow overriding the default backend via environment variable for testing
    // Usage: CAIPI_BACKEND=claudecli npm run tauri dev
    if let Ok(backend_env) = std::env::var("CAIPI_BACKEND") {
        if let Ok(kind) = backend_env.parse::<BackendKind>() {
            eprintln!("[init] Using backend from CAIPI_BACKEND env var: {}", kind);
            registry.set_default(kind);
        } else {
            eprintln!("[init] Warning: Unknown CAIPI_BACKEND value '{}', using default", backend_env);
        }
    }

    let registry = Arc::new(registry);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_os::init())
        .manage(session_store)
        .manage(permission_channels)
        .manage(registry)
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let app_handle = window.app_handle().clone();

                // Spawn cleanup task - we can't block here, but we give it a moment
                tauri::async_runtime::spawn(async move {
                    let sessions: tauri::State<'_, SessionStore> = app_handle.state();

                    // Drain sessions from store while holding lock briefly
                    let sessions_to_cleanup: Vec<(String, Arc<dyn BackendSession>)> = {
                        let mut store = sessions.lock().await;
                        store.drain().collect()
                    };
                    // Lock is now dropped

                    // Cleanup all sessions without holding the lock
                    for (id, session) in sessions_to_cleanup {
                        eprintln!("[cleanup] Cleaning up session: {}", id);
                        session.cleanup().await;
                    }

                    eprintln!("[cleanup] All sessions cleaned up");
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            // Setup commands
            commands::check_cli_status,
            commands::check_cli_installed,
            commands::check_cli_authenticated,
            commands::get_startup_info,
            commands::complete_onboarding,
            commands::reset_onboarding,
            commands::set_default_folder,
            commands::get_cli_path,
            commands::set_cli_path,
            // Folder commands
            commands::validate_folder,
            commands::get_recent_folders,
            commands::save_recent_folder,
            // Session commands
            commands::get_all_sessions,
            commands::get_recent_sessions,
            commands::get_project_sessions,
            commands::get_session_history,
            // File commands
            commands::list_directory,
            // Chat commands
            commands::create_session,
            commands::destroy_session,
            commands::send_message,
            commands::respond_permission,
            commands::get_session_messages,
            commands::abort_session,
            commands::set_permission_mode,
            commands::set_model,
            commands::set_thinking_level,
            // License commands
            commands::validate_license,
            commands::get_license_status,
            commands::clear_license,
            commands::revalidate_license_background,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
