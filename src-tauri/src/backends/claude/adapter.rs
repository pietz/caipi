//! Claude CLI backend adapter.
//!
//! This module implements a direct CLI wrapper that spawns `claude` as a subprocess
//! and communicates via JSON over stdin/stdout.
//!
//! Event handling logic (dispatching CLI events, permission hooks, etc.) lives in
//! the sibling `event_handler` module.

use async_trait::async_trait;
use std::collections::HashMap;
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, Notify, RwLock};
use tokio::task::JoinHandle;
use uuid::Uuid;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use crate::backends::session::BackendSession;
use crate::backends::types::{
    AuthStatus, Backend, BackendError, BackendKind, InstallStatus, SessionConfig,
};
use crate::backends::{emit_chat_event, PermissionChannels};
use super::cli_protocol::CliEvent;
use super::settings::{self, ClaudeSettings};
use crate::backends::types::{ChatEvent, Message};
use super::sessions::load_session_log_messages;
use crate::commands::setup::{check_cli_authenticated_internal, check_cli_installed_internal};

/// Claude backend implementation (CLI-backed).
pub struct ClaudeBackend;

impl ClaudeBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClaudeBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Backend for ClaudeBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Claude
    }

    async fn check_installed(&self) -> Result<InstallStatus, BackendError> {
        let status = check_cli_installed_internal().await;
        Ok(InstallStatus {
            installed: status.installed,
            version: status.version,
            path: status.path,
        })
    }

    async fn check_authenticated(&self) -> Result<AuthStatus, BackendError> {
        let status = check_cli_authenticated_internal().await;
        Ok(AuthStatus {
            authenticated: status.authenticated,
        })
    }

    async fn create_session(
        &self,
        config: SessionConfig,
        app_handle: AppHandle,
    ) -> Result<Arc<dyn BackendSession>, BackendError> {
        let session = CliSession::new(config, app_handle).await?;
        Ok(Arc::new(session))
    }
}

/// CLI session managing the claude subprocess.
pub struct CliSession {
    /// Unique session ID (either from CLI or generated)
    id: String,
    /// Folder path for this session
    folder_path: String,
    /// Current permission mode
    permission_mode: Arc<RwLock<String>>,
    /// Current model (what the user wants)
    model: Arc<RwLock<String>>,
    /// Model the running process was started with (for detecting model changes)
    process_model: Arc<RwLock<Option<String>>>,
    /// Current thinking/effort level (what the user wants; empty = CLI default)
    thinking_level: Arc<RwLock<String>>,
    /// Thinking level the running process was started with (for detecting changes)
    process_thinking_level: Arc<RwLock<Option<String>>>,
    /// CLI path override
    cli_path: Option<String>,
    /// Resume session ID (if resuming)
    resume_session_id: Option<String>,
    /// Tauri app handle for events
    app_handle: AppHandle,
    /// Whether an abort has been requested
    abort_flag: Arc<AtomicBool>,
    /// Notify for abort signaling
    abort_notify: Arc<Notify>,
    /// Best-effort: ensure one in-flight turn at a time (matches Codex behavior).
    in_flight: Arc<AtomicBool>,
    /// Frontend-generated turn id used for stale-event gating in the UI.
    current_turn_id: Arc<RwLock<Option<String>>>,
    /// User settings loaded from ~/.claude/settings.json
    user_settings: Option<ClaudeSettings>,
    /// Running CLI process (if any)
    process: Arc<Mutex<Option<Child>>>,
    /// Stdin writer for the CLI process
    stdin_writer: Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    /// Messages in this session
    messages: Arc<RwLock<Vec<Message>>>,
    /// Permission channels for user prompts
    permission_channels: PermissionChannels,
    /// CLI session ID (captured from init event, used for --resume)
    cli_session_id: Arc<RwLock<Option<String>>>,
    /// Background stderr drain task for the active process.
    stderr_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    /// Background stdout reader task for the active process.
    stdout_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    /// Background process lifecycle monitor task for the active process.
    lifecycle_task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl CliSession {
    fn format_exit_status(status: &ExitStatus) -> String {
        match status.code() {
            Some(code) => format!("exit code {}", code),
            None => "terminated by signal".to_string(),
        }
    }

    /// Monitor the current CLI process and clear stale handles on unexpected exit.
    async fn monitor_process_lifecycle(
        process: Arc<Mutex<Option<Child>>>,
        stdin_writer: Arc<Mutex<Option<tokio::process::ChildStdin>>>,
        abort_flag: Arc<AtomicBool>,
        in_flight: Arc<AtomicBool>,
        current_turn_id: Arc<RwLock<Option<String>>>,
        expected_pid: Option<u32>,
    ) -> Option<String> {
        let status = loop {
            let mut process_guard = process.lock().await;

            let Some(child) = process_guard.as_mut() else {
                // Process was explicitly cleared by abort/cleanup.
                return None;
            };

            // If a newer process replaced this one, stop monitoring.
            if expected_pid.is_some() && child.id() != expected_pid {
                return None;
            }

            match child.try_wait() {
                Ok(Some(status)) => {
                    *process_guard = None;
                    break status;
                }
                Ok(None) => {}
                Err(err) => {
                    // If process state can't be checked, clear handles so we can recover cleanly.
                    log::error!("Failed to poll Claude process status: {}", err);
                    *process_guard = None;
                    *stdin_writer.lock().await = None;

                    if !abort_flag.load(Ordering::SeqCst) {
                        in_flight.store(false, Ordering::SeqCst);
                        *current_turn_id.write().await = None;
                        return Some(
                            "Lost connection to Claude CLI process. Send another message to recover."
                                .to_string(),
                        );
                    }
                    return None;
                }
            }

            drop(process_guard);
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        };

        // Process exited - clear stale stdin handle.
        *stdin_writer.lock().await = None;

        let status_str = Self::format_exit_status(&status);
        log::warn!("Claude CLI process exited ({})", status_str);

        // Skip error UI for intentional abort path.
        if !abort_flag.load(Ordering::SeqCst) {
            in_flight.store(false, Ordering::SeqCst);
            *current_turn_id.write().await = None;
            return Some(format!(
                "Claude CLI process exited unexpectedly ({}). Send another message to resume.",
                status_str
            ));
        }
        None
    }

    async fn cleanup_process(&self) {
        self.abort_background_tasks().await;

        // Kill process if running
        let mut process_guard = self.process.lock().await;
        if let Some(ref mut child) = *process_guard {
            let _ = child.kill().await;
        }
        *process_guard = None;
        *self.stdin_writer.lock().await = None;
        *self.process_model.write().await = None;
        *self.process_thinking_level.write().await = None;
    }

    async fn abort_background_tasks(&self) {
        crate::backends::utils::abort_task_slot(&self.stderr_task).await;
        crate::backends::utils::abort_task_slot(&self.stdout_task).await;
        crate::backends::utils::abort_task_slot(&self.lifecycle_task).await;
    }

    /// Create a new CLI session.
    pub async fn new(config: SessionConfig, app_handle: AppHandle) -> Result<Self, BackendError> {
        let permission_mode = config
            .permission_mode
            .unwrap_or_else(|| "default".to_string());
        let model = config.model.unwrap_or_else(|| "sonnet".to_string());
        let user_settings = settings::load_user_settings();
        let initial_messages = config
            .resume_session_id
            .as_deref()
            .map(|session_id| {
                load_session_log_messages(&config.folder_path, session_id).unwrap_or_default()
            })
            .unwrap_or_default();

        // Get permission channels from app state
        let permission_channels: PermissionChannels =
            app_handle.state::<PermissionChannels>().inner().clone();

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            folder_path: config.folder_path,
            permission_mode: Arc::new(RwLock::new(permission_mode)),
            model: Arc::new(RwLock::new(model)),
            process_model: Arc::new(RwLock::new(None)),
            thinking_level: Arc::new(RwLock::new(String::new())),
            process_thinking_level: Arc::new(RwLock::new(None)),
            cli_path: config.cli_path,
            resume_session_id: config.resume_session_id,
            app_handle,
            abort_flag: Arc::new(AtomicBool::new(false)),
            abort_notify: Arc::new(Notify::new()),
            in_flight: Arc::new(AtomicBool::new(false)),
            current_turn_id: Arc::new(RwLock::new(None)),
            user_settings,
            process: Arc::new(Mutex::new(None)),
            stdin_writer: Arc::new(Mutex::new(None)),
            messages: Arc::new(RwLock::new(initial_messages)),
            permission_channels,
            cli_session_id: Arc::new(RwLock::new(None)),
            stderr_task: Arc::new(Mutex::new(None)),
            stdout_task: Arc::new(Mutex::new(None)),
            lifecycle_task: Arc::new(Mutex::new(None)),
        })
    }

    /// Spawn the CLI process with the given message.
    /// If `use_resume` is true and we have a CLI session ID, use --resume to preserve conversation.
    async fn spawn_cli(&self, message: &str) -> Result<(), BackendError> {
        self.spawn_cli_internal(message, false).await
    }

    /// Internal spawn with resume control.
    async fn spawn_cli_internal(
        &self,
        message: &str,
        use_resume: bool,
    ) -> Result<(), BackendError> {
        // Defensive: clear stale task handles before spawning another process.
        self.abort_background_tasks().await;

        let cli_cmd = self.cli_path.as_deref().unwrap_or("claude");
        let model = self.model.read().await.clone();
        let thinking_level = self.thinking_level.read().await.clone();
        let permission_mode = self.permission_mode.read().await.clone();

        // Store the model and thinking level we're spawning with
        *self.process_model.write().await = Some(model.clone());
        *self.process_thinking_level.write().await = Some(thinking_level.clone());

        let mut cmd = Command::new(cli_cmd);
        cmd.arg("-p")
            .arg("--output-format")
            .arg("stream-json")
            .arg("--verbose")
            .arg("--input-format")
            .arg("stream-json")
            .arg("--model")
            .arg(&model);

        // Append --effort if a thinking level is set (empty = let CLI use its default)
        if !thinking_level.is_empty() {
            cmd.arg("--effort").arg(&thinking_level);
        }

        // Map permission mode to CLI flag
        // Note: We only use --dangerously-skip-permissions for bypass mode.
        // For acceptEdits and default, we handle permissions via the control protocol hooks.
        // Don't use --allowedTools as it breaks Bash prompting behavior.
        if permission_mode == "bypassPermissions" {
            cmd.arg("--dangerously-skip-permissions");
        }
        // Note: --thinking is not a valid CLI flag. Extended thinking content comes
        // automatically in content blocks when the model uses it.

        // Add resume flag if resuming (either from config or for model switch)
        if use_resume {
            if let Some(ref session_id) = *self.cli_session_id.read().await {
                cmd.arg("--resume").arg(session_id);
            }
        } else if let Some(ref session_id) = self.resume_session_id {
            cmd.arg("--resume").arg(session_id);
        }

        // Clear env vars that prevent nested Claude Code sessions
        cmd.env_remove("CLAUDECODE");

        cmd.current_dir(&self.folder_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(target_os = "macos")]
        crate::backends::utils::add_homebrew_paths(&mut cmd);

        // On Windows, hide the console window
        #[cfg(target_os = "windows")]
        cmd.creation_flags(crate::backends::utils::CREATE_NO_WINDOW);

        let mut child = cmd.spawn().map_err(|e| BackendError {
            message: format!("Failed to spawn claude CLI: {}", e),
            recoverable: false,
        })?;

        // Take ownership of stdin
        let stdin = child.stdin.take().ok_or_else(|| BackendError {
            message: "Failed to capture CLI stdin".to_string(),
            recoverable: false,
        })?;

        // Take ownership of stdout for reading
        let stdout = child.stdout.take().ok_or_else(|| BackendError {
            message: "Failed to capture CLI stdout".to_string(),
            recoverable: false,
        })?;

        // Take ownership of stderr and drain it to prevent deadlock
        let stderr = child.stderr.take().ok_or_else(|| BackendError {
            message: "Failed to capture CLI stderr".to_string(),
            recoverable: false,
        })?;

        // Spawn a task to drain stderr (prevents deadlock on large output)
        let stderr_handle = crate::backends::utils::spawn_stderr_drain(stderr, "claude");
        *self.stderr_task.lock().await = Some(stderr_handle);

        let spawned_pid = child.id();

        // Store stdin writer and process handle
        *self.stdin_writer.lock().await = Some(stdin);
        *self.process.lock().await = Some(child);

        log::info!("Claude CLI spawned: pid={:?}, model={}, mode={}", spawned_pid, model, permission_mode);

        // Send initialize request with hooks before user message
        if let Err(e) = self.send_initialize().await {
            self.cleanup_process().await;
            return Err(e);
        }

        // Send the user message
        if let Err(e) = self.send_user_message(message).await {
            self.cleanup_process().await;
            return Err(e);
        }

        // Spawn task to read stdout and process events
        let app_handle = self.app_handle.clone();
        let abort_flag = self.abort_flag.clone();
        let abort_notify = self.abort_notify.clone();
        let in_flight = self.in_flight.clone();
        let current_turn_id = self.current_turn_id.clone();
        let permission_mode = self.permission_mode.clone();
        let user_settings = self.user_settings.clone();
        let permission_channels = self.permission_channels.clone();
        let stdin_writer = self.stdin_writer.clone();
        let session_id = self.id.clone();
        let session_id_for_stdout = session_id.clone();
        let session_id_for_lifecycle = session_id;
        let messages = self.messages.clone();
        let cli_session_id = self.cli_session_id.clone();
        let process = self.process.clone();

        let stdout_handle = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            // Track tool use IDs for status updates
            let mut active_tools: HashMap<String, String> = HashMap::new(); // tool_use_id -> tool_name

            loop {
                let line = match lines.next_line().await {
                    Ok(Some(line)) => line,
                    Ok(None) => break, // EOF
                    Err(err) => {
                        log::error!("Error reading Claude CLI stdout: {}", err);
                        break;
                    }
                };

                if abort_flag.load(Ordering::SeqCst) {
                    break;
                }

                // Best-effort: tag events with the currently active frontend turn id.
                let turn_id = current_turn_id.read().await.clone();

                // Skip empty lines
                if line.trim().is_empty() {
                    continue;
                }

                // Try to parse as JSON event
                match serde_json::from_str::<CliEvent>(&line) {
                    Ok(event) => {
                        Self::handle_event(
                            event,
                            &app_handle,
                            turn_id.as_deref(),
                            &permission_mode,
                            user_settings.as_ref(),
                            &permission_channels,
                            &stdin_writer,
                            &abort_flag,
                            &abort_notify,
                            &session_id_for_stdout,
                            &mut active_tools,
                            &messages,
                            &cli_session_id,
                            &in_flight,
                            &current_turn_id,
                        )
                        .await;
                    }
                    Err(e) => {
                        // Log parse errors but continue
                        log::warn!("Failed to parse Claude CLI event: {} - line: {}", e, line);
                    }
                }
            }
        });
        *self.stdout_task.lock().await = Some(stdout_handle);

        // Separate lifecycle monitor to detect dead processes and clear stale handles.
        let app_handle = self.app_handle.clone();
        let abort_flag = self.abort_flag.clone();
        let stdin_writer = self.stdin_writer.clone();
        let in_flight = self.in_flight.clone();
        let current_turn_id = self.current_turn_id.clone();
        let lifecycle_handle = tokio::spawn(async move {
            if let Some(message) =
                Self::monitor_process_lifecycle(
                    process,
                    stdin_writer,
                    abort_flag,
                    in_flight,
                    current_turn_id.clone(),
                    spawned_pid,
                )
                    .await
            {
                let turn_id = current_turn_id.read().await.clone();
                let error_event = ChatEvent::Error { message };
                emit_chat_event(
                    &app_handle,
                    Some(session_id_for_lifecycle.as_str()),
                    turn_id.as_deref(),
                    &error_event,
                );
            }
        });
        *self.lifecycle_task.lock().await = Some(lifecycle_handle);

        Ok(())
    }

    /// Write a JSON line to CLI stdin.
    async fn write_stdin_line(&self, json_value: &serde_json::Value) -> Result<(), BackendError> {
        let mut stdin_guard = self.stdin_writer.lock().await;
        if let Some(ref mut stdin) = *stdin_guard {
            let json_line = serde_json::to_string(json_value).map_err(|e| BackendError {
                message: format!("Failed to serialize message: {}", e),
                recoverable: false,
            })?;

            stdin
                .write_all(json_line.as_bytes())
                .await
                .map_err(|e| BackendError {
                    message: format!("Failed to write to CLI stdin: {}", e),
                    recoverable: false,
                })?;
            stdin.write_all(b"\n").await.map_err(|e| BackendError {
                message: format!("Failed to write newline to CLI stdin: {}", e),
                recoverable: false,
            })?;
            stdin.flush().await.map_err(|e| BackendError {
                message: format!("Failed to flush CLI stdin: {}", e),
                recoverable: false,
            })?;
            Ok(())
        } else {
            Err(BackendError {
                message: "CLI stdin not available".to_string(),
                recoverable: false,
            })
        }
    }

    /// Send the initialize control request with hooks.
    async fn send_initialize(&self) -> Result<(), BackendError> {
        let request_id = format!("req_init_{}", Uuid::new_v4());
        let init_request = serde_json::json!({
            "type": "control_request",
            "request_id": request_id,
            "request": {
                "subtype": "initialize",
                "hooks": {
                    "PreToolUse": [{
                        "matcher": null,
                        "hookCallbackIds": ["pretool_0"]
                    }],
                    "PostToolUse": [{
                        "matcher": null,
                        "hookCallbackIds": ["posttool_0"]
                    }]
                }
            }
        });
        self.write_stdin_line(&init_request).await
    }

    /// Send a user message to the CLI.
    async fn send_user_message(&self, content: &str) -> Result<(), BackendError> {
        // Get the CLI session ID if available
        let session_id = self.cli_session_id.read().await.clone();

        let user_message = serde_json::json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": content
            },
            "session_id": session_id.as_deref().unwrap_or("default")
        });
        self.write_stdin_line(&user_message).await
    }
}

#[async_trait]
impl BackendSession for CliSession {
    fn session_id(&self) -> &str {
        &self.id
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::Claude
    }

    fn folder_path(&self) -> &str {
        &self.folder_path
    }

    async fn send_message(
        &self,
        message: &str,
        turn_id: Option<&str>,
    ) -> Result<(), BackendError> {
        if self.in_flight.swap(true, Ordering::SeqCst) {
            return Err(BackendError {
                message: "Claude session is busy".to_string(),
                recoverable: true,
            });
        }

        *self.current_turn_id.write().await = turn_id.map(|id| id.to_string());

        // Clear abort flag at start of new message (allows recovery after abort)
        self.abort_flag.store(false, Ordering::SeqCst);

        // Store user message
        {
            let mut msgs = self.messages.write().await;
            msgs.push(Message::new("user", message));
        }

        // Check if we have an active process
        let has_process = self.process.lock().await.is_some();

        // Check if model or thinking level changed since process was spawned
        let current_model = self.model.read().await.clone();
        let process_model = self.process_model.read().await.clone();
        let model_changed = has_process && process_model.as_ref() != Some(&current_model);

        let current_thinking = self.thinking_level.read().await.clone();
        let process_thinking = self.process_thinking_level.read().await.clone();
        let thinking_changed = has_process && process_thinking.as_ref() != Some(&current_thinking);

        let needs_respawn = model_changed || thinking_changed;

        log::debug!("Message routing: has_process={}, model_changed={}, thinking_changed={}, has_cli_session={}", has_process, model_changed, thinking_changed, self.cli_session_id.read().await.is_some());

        let result = if needs_respawn {
            // Model or thinking level changed - kill old process and respawn with --resume to preserve conversation
            self.cleanup_process().await;
            self.spawn_cli_internal(message, true).await
        } else if has_process {
            // Send message to existing process
            self.send_user_message(message).await
        } else {
            // Spawn new CLI process. If this session had a previous CLI session ID
            // (e.g. after abort), resume it to preserve conversation context.
            let has_cli_session = self.cli_session_id.read().await.is_some();
            if has_cli_session {
                self.spawn_cli_internal(message, true).await
            } else {
                self.spawn_cli(message).await
            }
        };

        if result.is_err() {
            self.in_flight.store(false, Ordering::SeqCst);
            *self.current_turn_id.write().await = None;
        }

        result
    }

    async fn abort(&self) -> Result<(), BackendError> {
        let turn_id = self.current_turn_id.read().await.clone();

        // Set abort flag
        self.abort_flag.store(true, Ordering::SeqCst);
        self.abort_notify.notify_waiters();

        // Send interrupt control request to give CLI a chance to stop gracefully
        let interrupt_request = serde_json::json!({
            "type": "control_request",
            "request_id": format!("req_int_{}", Uuid::new_v4()),
            "request": {
                "subtype": "interrupt"
            }
        });
        let _ = self.write_stdin_line(&interrupt_request).await;

        // Give CLI a moment to respond gracefully
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Stop background tasks and process resources.
        self.cleanup_process().await;

        self.in_flight.store(false, Ordering::SeqCst);
        *self.current_turn_id.write().await = None;

        // Emit abort complete
        let abort_complete = ChatEvent::AbortComplete {
            session_id: self.id.clone(),
        };
        emit_chat_event(
            &self.app_handle,
            Some(self.id.as_str()),
            turn_id.as_deref(),
            &abort_complete,
        );

        Ok(())
    }

    async fn cleanup(&self) {
        self.cleanup_process().await;
        self.in_flight.store(false, Ordering::SeqCst);
        *self.current_turn_id.write().await = None;
    }

    async fn get_permission_mode(&self) -> String {
        self.permission_mode.read().await.clone()
    }

    async fn set_permission_mode(&self, mode: String) -> Result<(), BackendError> {
        *self.permission_mode.write().await = mode;
        Ok(())
    }

    async fn get_model(&self) -> String {
        self.model.read().await.clone()
    }

    async fn set_model(&self, model: String) -> Result<(), BackendError> {
        *self.model.write().await = model;
        Ok(())
    }

    async fn set_thinking_level(&self, level: String) -> Result<(), BackendError> {
        *self.thinking_level.write().await = level;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::cli_protocol::UsageInfo;
    use std::sync::atomic::AtomicBool;
    use tokio::process::Command;
    use tokio::sync::{Mutex, RwLock};

    async fn spawn_fast_exit_process(code: i32) -> Child {
        #[cfg(target_os = "windows")]
        {
            Command::new("cmd")
                .args(["/C", &format!("exit {}", code)])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn windows child")
        }

        #[cfg(not(target_os = "windows"))]
        {
            Command::new("sh")
                .args(["-c", &format!("exit {}", code)])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn unix child")
        }
    }

    #[test]
    fn test_cli_backend_kind() {
        let backend = ClaudeBackend::new();
        assert_eq!(backend.kind(), BackendKind::Claude);
    }

    #[test]
    fn test_context_tokens_from_usage_counts_all_input_sides() {
        let usage = UsageInfo {
            input_tokens: 100,
            output_tokens: 50,
            cache_read_input_tokens: 20,
            cache_creation_input_tokens: 10,
        };

        assert_eq!(CliSession::context_tokens_from_usage(&usage), 130);
    }

    #[tokio::test]
    async fn monitor_process_lifecycle_clears_handles_and_returns_error_when_not_aborted() {
        let child = spawn_fast_exit_process(9).await;
        let pid = child.id();
        let process = Arc::new(Mutex::new(Some(child)));
        let stdin_writer = Arc::new(Mutex::new(None));
        let abort_flag = Arc::new(AtomicBool::new(false));
        let in_flight = Arc::new(AtomicBool::new(true));
        let current_turn_id = Arc::new(RwLock::new(Some("turn-1".to_string())));

        let message = CliSession::monitor_process_lifecycle(
            process.clone(),
            stdin_writer.clone(),
            abort_flag,
            in_flight.clone(),
            current_turn_id.clone(),
            pid,
        )
        .await;

        assert!(process.lock().await.is_none());
        assert!(stdin_writer.lock().await.is_none());
        assert!(message.is_some());
        assert!(message.unwrap().contains("exited unexpectedly"));
        assert!(!in_flight.load(Ordering::SeqCst));
        assert!(current_turn_id.read().await.is_none());
    }

    #[tokio::test]
    async fn monitor_process_lifecycle_suppresses_error_when_aborted() {
        let child = spawn_fast_exit_process(0).await;
        let pid = child.id();
        let process = Arc::new(Mutex::new(Some(child)));
        let stdin_writer = Arc::new(Mutex::new(None));
        let abort_flag = Arc::new(AtomicBool::new(true));
        let in_flight = Arc::new(AtomicBool::new(true));
        let current_turn_id = Arc::new(RwLock::new(Some("turn-2".to_string())));

        let message = CliSession::monitor_process_lifecycle(
            process.clone(),
            stdin_writer.clone(),
            abort_flag,
            in_flight,
            current_turn_id,
            pid,
        )
        .await;

        assert!(process.lock().await.is_none());
        assert!(stdin_writer.lock().await.is_none());
        assert!(message.is_none());
    }
}
