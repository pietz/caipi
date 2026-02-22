pub mod backends;
mod commands;
mod storage;

use backends::claude::ClaudeBackend;
use backends::codex::CodexBackend;
use backends::PermissionChannels;
use backends::{BackendKind, BackendRegistry, BackendSession};
use commands::chat::SessionStore;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Manager;
use tauri_plugin_log::{Target, TargetKind};
use tokio::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let session_store: SessionStore = Arc::new(Mutex::new(HashMap::new()));
    let permission_channels: PermissionChannels = Arc::new(Mutex::new(HashMap::new()));

    // Initialize backend registry
    let mut registry = BackendRegistry::new();
    registry.register(Arc::new(ClaudeBackend::new()));
    registry.register(Arc::new(CodexBackend::new()));

    // Allow overriding the default backend via environment variable for testing
    // Usage: CAIPI_BACKEND=claude npm run tauri dev
    if let Ok(backend_env) = std::env::var("CAIPI_BACKEND") {
        if let Ok(kind) = backend_env.parse::<BackendKind>() {
            eprintln!("[init] Using backend from CAIPI_BACKEND env var: {}", kind);
            registry.set_default(kind);
        } else {
            eprintln!(
                "[init] Warning: Unknown CAIPI_BACKEND value '{}', using default",
                backend_env
            );
        }
    }

    let registry = Arc::new(registry);
    let close_in_progress = Arc::new(AtomicBool::new(false));

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Debug)
                .max_file_size(10_000_000)
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::Webview),
                    Target::new(TargetKind::LogDir { file_name: None }),
                ])
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_os::init())
        .manage(session_store)
        .manage(permission_channels)
        .manage(registry)
        .on_window_event({
            let close_in_progress = close_in_progress.clone();
            move |window, event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();

                    // Cleanup already in progress – just keep blocking.
                    if close_in_progress.swap(true, Ordering::SeqCst) {
                        return;
                    }

                    let window_to_close = window.clone();
                    let app_handle = window.app_handle().clone();

                    tauri::async_runtime::spawn(async move {
                        let sessions: tauri::State<'_, SessionStore> = app_handle.state();

                        // Drain sessions from store while holding lock briefly.
                        let sessions_to_cleanup: Vec<(String, Arc<dyn BackendSession>)> = {
                            let mut store = sessions.lock().await;
                            store.drain().collect()
                        };

                        // Cleanup all sessions without holding the store lock.
                        for (id, session) in sessions_to_cleanup {
                            log::info!("Cleaning up session: {}", id);
                            let _ = tokio::time::timeout(
                                std::time::Duration::from_secs(3),
                                session.cleanup(),
                            )
                            .await;
                        }

                        log::info!("All sessions cleaned up");

                        // Trigger close again; this time the event is allowed.
                        let _ = window_to_close.close();
                    });
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            // Setup commands
            commands::check_all_backends_status,
            commands::get_startup_info,
            commands::complete_onboarding,
            commands::set_default_backend,
            commands::get_backend_cli_path,
            commands::set_backend_cli_path,
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
            commands::abort_session,
            commands::set_permission_mode,
            commands::set_model,
            commands::set_thinking_level,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
