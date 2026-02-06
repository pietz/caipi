//! Claude CLI direct wrapper backend adapter.
//!
//! This module implements a direct CLI wrapper that spawns `claude` as a subprocess
//! and communicates via JSON over stdin/stdout, providing an alternative to the SDK.

use async_trait::async_trait;
use std::collections::HashMap;
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, Notify, RwLock};
use uuid::Uuid;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

use crate::backends::session::BackendSession;
use crate::backends::types::{
    AuthStatus, Backend, BackendCapabilities, BackendError, BackendKind, InstallStatus, ModelInfo,
    PermissionModel, SessionConfig,
};
use crate::backends::{PermissionChannels, CHAT_EVENT_CHANNEL};
use crate::claude::cli_protocol::{
    AssistantEvent, CliEvent, ContentBlock, IncomingControlRequest, OutgoingControlResponse,
    ResultEvent, SystemEvent, UsageInfo,
};
use crate::claude::hooks::{determine_permission, PermissionDecision};
use crate::claude::settings::{self, ClaudeSettings};
use crate::claude::tool_utils::extract_tool_target;
use crate::commands::chat::{ChatEvent, Message};
use crate::commands::sessions::load_session_log_messages;
use crate::commands::setup::{check_cli_authenticated_internal, check_cli_installed_internal};

/// Claude CLI backend implementation.
pub struct ClaudeCliBackend;

impl ClaudeCliBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClaudeCliBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Backend for ClaudeCliBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::ClaudeCli
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            permission_model: PermissionModel::PerOperation,
            supports_streaming: true,
            supports_abort: true,
            supports_resume: true,
            supports_extended_thinking: true,
            available_models: vec![
                ModelInfo {
                    id: "opus".to_string(),
                    name: "Claude Opus 4.6".to_string(),
                    supports_thinking: true,
                },
                ModelInfo {
                    id: "sonnet".to_string(),
                    name: "Claude Sonnet 4.5".to_string(),
                    supports_thinking: true,
                },
                ModelInfo {
                    id: "haiku".to_string(),
                    name: "Claude Haiku 4.5".to_string(),
                    supports_thinking: false,
                },
            ],
        }
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
                    eprintln!("[cli_adapter] Failed to poll process status: {}", err);
                    *process_guard = None;
                    *stdin_writer.lock().await = None;

                    if !abort_flag.load(Ordering::SeqCst) {
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
        eprintln!("[cli_adapter] Claude CLI process exited ({})", status_str);

        // Skip error UI for intentional abort path.
        if !abort_flag.load(Ordering::SeqCst) {
            return Some(format!(
                "Claude CLI process exited unexpectedly ({}). Send another message to resume.",
                status_str
            ));
        }
        None
    }

    /// Compute context usage for UI from assistant usage.
    /// This tracks effective input-side context load for the current call.
    fn context_tokens_from_usage(usage: &UsageInfo) -> u64 {
        usage.input_tokens + usage.cache_read_input_tokens + usage.cache_creation_input_tokens
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
            .map(|session_id| load_session_log_messages(&config.folder_path, session_id).unwrap_or_default())
            .unwrap_or_default();

        // Get permission channels from app state
        let permission_channels: PermissionChannels = app_handle.state::<PermissionChannels>().inner().clone();

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            folder_path: config.folder_path,
            permission_mode: Arc::new(RwLock::new(permission_mode)),
            model: Arc::new(RwLock::new(model)),
            process_model: Arc::new(RwLock::new(None)),
            cli_path: config.cli_path,
            resume_session_id: config.resume_session_id,
            app_handle,
            abort_flag: Arc::new(AtomicBool::new(false)),
            abort_notify: Arc::new(Notify::new()),
            user_settings,
            process: Arc::new(Mutex::new(None)),
            stdin_writer: Arc::new(Mutex::new(None)),
            messages: Arc::new(RwLock::new(initial_messages)),
            permission_channels,
            cli_session_id: Arc::new(RwLock::new(None)),
        })
    }

    /// Spawn the CLI process with the given message.
    /// If `use_resume` is true and we have a CLI session ID, use --resume to preserve conversation.
    async fn spawn_cli(&self, message: &str) -> Result<(), BackendError> {
        self.spawn_cli_internal(message, false).await
    }

    /// Internal spawn with resume control.
    async fn spawn_cli_internal(&self, message: &str, use_resume: bool) -> Result<(), BackendError> {
        let cli_cmd = self.cli_path.as_deref().unwrap_or("claude");
        let model = self.model.read().await.clone();
        let permission_mode = self.permission_mode.read().await.clone();

        // Store the model we're spawning with
        *self.process_model.write().await = Some(model.clone());

        let mut cmd = Command::new(cli_cmd);
        cmd.arg("-p")
            .arg("--output-format")
            .arg("stream-json")
            .arg("--verbose")
            .arg("--input-format")
            .arg("stream-json")
            .arg("--model")
            .arg(&model);

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

        cmd.current_dir(&self.folder_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // On Windows, hide the console window
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

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
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            use tokio::io::AsyncBufReadExt;
            while let Ok(n) = reader.read_line(&mut line).await {
                if n == 0 {
                    break;
                }
                // Log stderr for debugging, but don't let it block
                if !line.trim().is_empty() {
                    eprintln!("[claude stderr] {}", line.trim());
                }
                line.clear();
            }
        });

        let spawned_pid = child.id();

        // Store stdin writer and process handle
        *self.stdin_writer.lock().await = Some(stdin);
        *self.process.lock().await = Some(child);

        // Send initialize request with hooks before user message
        self.send_initialize().await?;

        // Send the user message
        self.send_user_message(message).await?;

        // Spawn task to read stdout and process events
        let app_handle = self.app_handle.clone();
        let abort_flag = self.abort_flag.clone();
        let abort_notify = self.abort_notify.clone();
        let permission_mode = self.permission_mode.clone();
        let user_settings = self.user_settings.clone();
        let permission_channels = self.permission_channels.clone();
        let stdin_writer = self.stdin_writer.clone();
        let session_id = self.id.clone();
        let messages = self.messages.clone();
        let cli_session_id = self.cli_session_id.clone();
        let process = self.process.clone();

        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            // Track tool use IDs for status updates
            let mut active_tools: HashMap<String, String> = HashMap::new(); // tool_use_id -> tool_name

            loop {
                let line = match lines.next_line().await {
                    Ok(Some(line)) => line,
                    Ok(None) => break, // EOF
                    Err(err) => {
                        eprintln!("[cli_adapter] Error reading CLI stdout: {}", err);
                        break;
                    }
                };

                if abort_flag.load(Ordering::SeqCst) {
                    break;
                }

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
                            &permission_mode,
                            user_settings.as_ref(),
                            &permission_channels,
                            &stdin_writer,
                            &abort_flag,
                            &abort_notify,
                            &session_id,
                            &mut active_tools,
                            &messages,
                            &cli_session_id,
                        )
                        .await;
                    }
                    Err(e) => {
                        // Log parse errors but continue
                        eprintln!("[cli_adapter] Failed to parse event: {} - line: {}", e, line);
                    }
                }
            }
        });

        // Separate lifecycle monitor to detect dead processes and clear stale handles.
        let app_handle = self.app_handle.clone();
        let abort_flag = self.abort_flag.clone();
        let stdin_writer = self.stdin_writer.clone();
        tokio::spawn(async move {
            if let Some(message) = Self::monitor_process_lifecycle(
                process,
                stdin_writer,
                abort_flag,
                spawned_pid,
            )
            .await
            {
                let _ = app_handle.emit(CHAT_EVENT_CHANNEL, &ChatEvent::Error { message });
            }
        });

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

    /// Send a control response to CLI stdin.
    async fn send_control_response(
        stdin_writer: &Arc<Mutex<Option<tokio::process::ChildStdin>>>,
        response: OutgoingControlResponse,
    ) -> Result<(), String> {
        let mut stdin_guard = stdin_writer.lock().await;
        if let Some(ref mut stdin) = *stdin_guard {
            let json_line = serde_json::to_string(&response)
                .map_err(|e| format!("Failed to serialize control response: {}", e))?;

            stdin
                .write_all(json_line.as_bytes())
                .await
                .map_err(|e| format!("Failed to write control response: {}", e))?;
            stdin
                .write_all(b"\n")
                .await
                .map_err(|e| format!("Failed to write newline: {}", e))?;
            stdin
                .flush()
                .await
                .map_err(|e| format!("Failed to flush stdin: {}", e))?;
            Ok(())
        } else {
            Err("CLI stdin not available".to_string())
        }
    }

    /// Handle a CLI event.
    #[allow(clippy::too_many_arguments)]
    async fn handle_event(
        event: CliEvent,
        app_handle: &AppHandle,
        permission_mode: &Arc<RwLock<String>>,
        user_settings: Option<&ClaudeSettings>,
        permission_channels: &PermissionChannels,
        stdin_writer: &Arc<Mutex<Option<tokio::process::ChildStdin>>>,
        abort_flag: &Arc<AtomicBool>,
        abort_notify: &Arc<Notify>,
        session_id: &str,
        active_tools: &mut HashMap<String, String>,
        messages: &Arc<RwLock<Vec<Message>>>,
        cli_session_id: &Arc<RwLock<Option<String>>>,
    ) {
        match event {
            CliEvent::System(system) => {
                Self::handle_system_event(system, app_handle, session_id, cli_session_id).await;
            }
            CliEvent::Assistant(assistant) => {
                Self::handle_assistant_event(
                    assistant,
                    app_handle,
                    active_tools,
                    messages,
                )
                .await;
            }
            CliEvent::User(_user) => {
                Self::handle_user_event(_user, app_handle, active_tools).await;
            }
            CliEvent::Result(result) => {
                Self::handle_result_event(result, app_handle).await;
            }
            CliEvent::ControlRequest(request) => {
                Self::handle_control_request(
                    request,
                    app_handle,
                    permission_mode,
                    user_settings,
                    permission_channels,
                    stdin_writer,
                    abort_flag,
                    abort_notify,
                    session_id,
                    active_tools,
                )
                .await;
            }
            CliEvent::ControlResponse(_ack) => {
                // Acknowledgment of our control response - nothing to do
            }
        }
    }

    /// Handle system events (init, health_check).
    async fn handle_system_event(
        event: SystemEvent,
        app_handle: &AppHandle,
        _session_id: &str,
        cli_session_id: &Arc<RwLock<Option<String>>>,
    ) {
        if event.subtype == "init" {
            // Capture CLI session ID for message correlation
            if let Some(sid) = event.session_id {
                *cli_session_id.write().await = Some(sid);
            }

            // Parse apiKeySource from init event data
            let api_key_source = event
                .data
                .get("apiKeySource")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            let auth_type = match api_key_source {
                "none" => "Claude AI Subscription",
                "environment" | "settings" => "Anthropic API Key",
                _ => "Unknown",
            }
            .to_string();

            let _ = app_handle.emit(CHAT_EVENT_CHANNEL, &ChatEvent::SessionInit { auth_type });
        }
    }

    /// Handle assistant events (messages with content blocks).
    /// Note: ToolStart is now emitted from hook callbacks, not from tool_use blocks.
    async fn handle_assistant_event(
        event: AssistantEvent,
        app_handle: &AppHandle,
        active_tools: &mut HashMap<String, String>,
        messages: &Arc<RwLock<Vec<Message>>>,
    ) {
        if let Some(usage) = &event.message.usage {
            let total_tokens = Self::context_tokens_from_usage(usage);
            let _ = app_handle.emit(CHAT_EVENT_CHANNEL, &ChatEvent::TokenUsage { total_tokens });
        }

        for block in event.message.content {
            match block {
                ContentBlock::Text(text_block) => {
                    let _ = app_handle.emit(
                        CHAT_EVENT_CHANNEL,
                        &ChatEvent::Text {
                            content: text_block.text.clone(),
                        },
                    );

                    // Store message
                    let mut msgs = messages.write().await;
                    msgs.push(Message {
                        id: Uuid::new_v4().to_string(),
                        role: "assistant".to_string(),
                        content: text_block.text,
                        timestamp: chrono::Utc::now().timestamp(),
                    });
                }
                ContentBlock::Thinking(thinking_block) => {
                    let thinking_id = Uuid::new_v4().to_string();
                    let _ = app_handle.emit(
                        CHAT_EVENT_CHANNEL,
                        &ChatEvent::ThinkingStart {
                            thinking_id: thinking_id.clone(),
                            content: thinking_block.thinking,
                        },
                    );
                    let _ = app_handle.emit(
                        CHAT_EVENT_CHANNEL,
                        &ChatEvent::ThinkingEnd {
                            thinking_id,
                        },
                    );
                }
                ContentBlock::ToolUse(tool_use) => {
                    // Track the tool for ToolEnd matching
                    // ToolStart is emitted from the PreToolUse hook callback, not here
                    active_tools.insert(tool_use.id.clone(), tool_use.name.clone());
                }
                ContentBlock::ToolResult(tool_result) => {
                    // Tool completed. Guard on active_tools to avoid duplicate ToolEnd
                    // emissions if both User and Assistant streams include tool_result.
                    if active_tools.remove(&tool_result.tool_use_id).is_some() {
                        let status = if tool_result.is_error {
                            "error"
                        } else {
                            "completed"
                        };
                        let _ = app_handle.emit(
                            CHAT_EVENT_CHANNEL,
                            &ChatEvent::ToolEnd {
                                id: tool_result.tool_use_id.clone(),
                                status: status.to_string(),
                            },
                        );
                    }
                }
                ContentBlock::InputJsonDelta(_) => {
                    // Streaming delta - we can ignore for now since we get the complete input later
                }
            }
        }
        // Token usage is emitted from assistant usage (per call) to represent
        // context usage, not cumulative session totals.
    }

    /// Handle user events (tool results).
    async fn handle_user_event(
        event: crate::claude::cli_protocol::UserEvent,
        app_handle: &AppHandle,
        active_tools: &mut HashMap<String, String>,
    ) {
        if let Some(message) = event.extra.get("message") {
            if let Some(content_array) = message.get("content").and_then(|c| c.as_array()) {
                for item in content_array {
                    if item.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                        if let Some(tool_use_id) = item.get("tool_use_id").and_then(|id| id.as_str()) {
                            // Emit once per tool ID. Assistant blocks may also contain tool_result
                            // in some protocol variants.
                            if active_tools.remove(tool_use_id).is_some() {
                                let is_error = item.get("is_error").and_then(|e| e.as_bool()).unwrap_or(false);
                                let status = if is_error { "error" } else { "completed" };
                                let _ = app_handle.emit(
                                    CHAT_EVENT_CHANNEL,
                                    &ChatEvent::ToolEnd {
                                        id: tool_use_id.to_string(),
                                        status: status.to_string(),
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// Handle a control request from the CLI (hook callbacks).
    /// This is where ToolStart is emitted and permissions are determined.
    #[allow(clippy::too_many_arguments)]
    async fn handle_control_request(
        request: IncomingControlRequest,
        app_handle: &AppHandle,
        permission_mode: &Arc<RwLock<String>>,
        user_settings: Option<&ClaudeSettings>,
        permission_channels: &PermissionChannels,
        stdin_writer: &Arc<Mutex<Option<tokio::process::ChildStdin>>>,
        abort_flag: &Arc<AtomicBool>,
        abort_notify: &Arc<Notify>,
        _session_id: &str,
        active_tools: &mut HashMap<String, String>,
    ) {
        // Handle different control request types
        if request.request.subtype == "hook_callback" {
            if let Some(input) = &request.request.input {
                if input.hook_event_name == "PreToolUse" {
                    Self::handle_pretool_hook(
                        &request,
                        input,
                        app_handle,
                        permission_mode,
                        user_settings,
                        permission_channels,
                        stdin_writer,
                        abort_flag,
                        abort_notify,
                        active_tools,
                    )
                    .await;
                } else if input.hook_event_name == "PostToolUse" {
                    // Acknowledge PostToolUse.
                    // Tool completion is emitted from ToolResult blocks so we preserve
                    // the real final status (including errors) and avoid duplicate ToolEnd events.
                    let response = OutgoingControlResponse::ack_posttool(request.request_id.clone());
                    if let Err(e) = Self::send_control_response(stdin_writer, response).await {
                        eprintln!("[cli_adapter] Failed to send PostToolUse ack: {}", e);
                    }
                }
            }
        }
    }

    /// Handle a PreToolUse hook callback - emit ToolStart and determine permission.
    #[allow(clippy::too_many_arguments)]
    async fn handle_pretool_hook(
        request: &IncomingControlRequest,
        input: &crate::claude::cli_protocol::HookCallbackInput,
        app_handle: &AppHandle,
        permission_mode: &Arc<RwLock<String>>,
        user_settings: Option<&ClaudeSettings>,
        permission_channels: &PermissionChannels,
        stdin_writer: &Arc<Mutex<Option<tokio::process::ChildStdin>>>,
        abort_flag: &Arc<AtomicBool>,
        abort_notify: &Arc<Notify>,
        active_tools: &mut HashMap<String, String>,
    ) {
        let tool_name = input.tool_name.clone().unwrap_or_default();
        let tool_input = input.tool_input.clone().unwrap_or(serde_json::json!({}));
        let tool_use_id = request.request.tool_use_id.clone().unwrap_or_else(|| Uuid::new_v4().to_string());

        // Track this tool
        active_tools.insert(tool_use_id.clone(), tool_name.clone());

        // Extract target for display
        let target = extract_tool_target(&tool_name, &tool_input);

        // Emit ToolStart with pending status
        let input_for_frontend = if tool_name.starts_with("Task") || tool_name.starts_with("Todo") {
            Some(tool_input.clone())
        } else {
            None
        };

        let _ = app_handle.emit(
            CHAT_EVENT_CHANNEL,
            &ChatEvent::ToolStart {
                tool_use_id: tool_use_id.clone(),
                tool_type: tool_name.clone(),
                target,
                status: "pending".to_string(),
                input: input_for_frontend,
            },
        );

        // Check abort first
        if abort_flag.load(Ordering::SeqCst) {
            // Remove from active_tools since tool won't run
            active_tools.remove(&tool_use_id);
            let response = OutgoingControlResponse::deny_pretool(
                request.request_id.clone(),
                "Session aborted",
            );
            if let Err(e) = Self::send_control_response(stdin_writer, response).await {
                eprintln!("[cli_adapter] Failed to send abort response: {}", e);
            }
            return;
        }

        // Determine permission
        let current_mode = permission_mode.read().await.clone();
        let decision = determine_permission(&current_mode, &tool_name, &tool_input, user_settings);

        match decision {
            PermissionDecision::Allow(reason) => {
                // Auto-approved - emit running status and send allow response
                let _ = app_handle.emit(
                    CHAT_EVENT_CHANNEL,
                    &ChatEvent::ToolStatusUpdate {
                        tool_use_id: tool_use_id.clone(),
                        status: "running".to_string(),
                        permission_request_id: None,
                    },
                );

                let response = OutgoingControlResponse::allow_pretool(
                    request.request_id.clone(),
                    &reason,
                );
                if let Err(e) = Self::send_control_response(stdin_writer, response).await {
                    eprintln!("[cli_adapter] Failed to send allow response: {}", e);
                }
            }
            PermissionDecision::Deny(reason) => {
                // Denied - remove from active_tools, emit denied status, send deny response
                active_tools.remove(&tool_use_id);
                let _ = app_handle.emit(
                    CHAT_EVENT_CHANNEL,
                    &ChatEvent::ToolStatusUpdate {
                        tool_use_id: tool_use_id.clone(),
                        status: "denied".to_string(),
                        permission_request_id: None,
                    },
                );

                let response = OutgoingControlResponse::deny_pretool(
                    request.request_id.clone(),
                    &reason,
                );
                if let Err(e) = Self::send_control_response(stdin_writer, response).await {
                    eprintln!("[cli_adapter] Failed to send deny response: {}", e);
                }
            }
            PermissionDecision::PromptUser => {
                // Need to prompt user - set up permission channel and wait
                let permission_request_id = Uuid::new_v4().to_string();
                let (tx, rx) = tokio::sync::oneshot::channel();

                // Store sender in permission channels
                {
                    let mut channels = permission_channels.lock().await;
                    channels.insert(permission_request_id.clone(), tx);
                }

                // Emit awaiting_permission status
                let _ = app_handle.emit(
                    CHAT_EVENT_CHANNEL,
                    &ChatEvent::ToolStatusUpdate {
                        tool_use_id: tool_use_id.clone(),
                        status: "awaiting_permission".to_string(),
                        permission_request_id: Some(permission_request_id.clone()),
                    },
                );

                // Wait for user response with timeout and abort support
                let timeout = tokio::time::sleep(std::time::Duration::from_secs(60));
                tokio::pin!(timeout);
                tokio::pin!(rx);

                let (allowed, reason) = tokio::select! {
                    response = &mut rx => {
                        match response {
                            Ok(r) if r.allowed => (true, "User approved".to_string()),
                            Ok(_) => (false, "User denied".to_string()),
                            Err(_) => (false, "Permission request cancelled".to_string()),
                        }
                    }
                    _ = &mut timeout => {
                        (false, "Permission request timed out".to_string())
                    }
                    _ = abort_notify.notified() => {
                        (false, "Session aborted".to_string())
                    }
                };

                // Cleanup channel
                {
                    let mut channels = permission_channels.lock().await;
                    channels.remove(&permission_request_id);
                }

                // If denied, remove from active_tools since tool won't run
                if !allowed {
                    active_tools.remove(&tool_use_id);
                }

                // Emit final status and send control response
                let status = if allowed { "running" } else { "denied" };
                let _ = app_handle.emit(
                    CHAT_EVENT_CHANNEL,
                    &ChatEvent::ToolStatusUpdate {
                        tool_use_id: tool_use_id.clone(),
                        status: status.to_string(),
                        permission_request_id: None,
                    },
                );

                let response = if allowed {
                    OutgoingControlResponse::allow_pretool(request.request_id.clone(), &reason)
                } else {
                    OutgoingControlResponse::deny_pretool(request.request_id.clone(), &reason)
                };
                if let Err(e) = Self::send_control_response(stdin_writer, response).await {
                    eprintln!("[cli_adapter] Failed to send permission response: {}", e);
                }
            }
        }
    }

    /// Handle result events (completion).
    async fn handle_result_event(event: ResultEvent, app_handle: &AppHandle) {
        if event.subtype == "success" {
            let _ = app_handle.emit(CHAT_EVENT_CHANNEL, &ChatEvent::Complete);
        } else if event.subtype == "error" {
            let _ = app_handle.emit(
                CHAT_EVENT_CHANNEL,
                &ChatEvent::Error {
                    message: "CLI returned error".to_string(),
                },
            );
        }

        // Do not emit token usage from result totals here: result usage is
        // cumulative session accounting and does not match context usage semantics.
    }
}

#[async_trait]
impl BackendSession for CliSession {
    fn session_id(&self) -> &str {
        &self.id
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::ClaudeCli
    }

    fn folder_path(&self) -> &str {
        &self.folder_path
    }

    async fn send_message(&self, message: &str) -> Result<(), BackendError> {
        // Clear abort flag at start of new message (allows recovery after abort)
        self.abort_flag.store(false, Ordering::SeqCst);

        // Store user message
        {
            let mut msgs = self.messages.write().await;
            msgs.push(Message {
                id: Uuid::new_v4().to_string(),
                role: "user".to_string(),
                content: message.to_string(),
                timestamp: chrono::Utc::now().timestamp(),
            });
        }

        // Check if we have an active process
        let has_process = self.process.lock().await.is_some();

        // Check if model changed since process was spawned
        let current_model = self.model.read().await.clone();
        let process_model = self.process_model.read().await.clone();
        let model_changed = has_process && process_model.as_ref() != Some(&current_model);

        if model_changed {
            // Model changed - kill old process and respawn with --resume to preserve conversation
            self.cleanup().await;
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
        }
    }

    async fn abort(&self) -> Result<(), BackendError> {
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

        // Kill the CLI process if still running
        let mut process_guard = self.process.lock().await;
        if let Some(ref mut child) = *process_guard {
            let _ = child.kill().await;
        }
        *process_guard = None;

        // Clear stdin writer
        *self.stdin_writer.lock().await = None;

        // Emit abort complete
        let _ = self.app_handle.emit(
            CHAT_EVENT_CHANNEL,
            &ChatEvent::AbortComplete {
                session_id: self.id.clone(),
            },
        );

        Ok(())
    }

    async fn cleanup(&self) {
        // Kill process if running
        let mut process_guard = self.process.lock().await;
        if let Some(ref mut child) = *process_guard {
            let _ = child.kill().await;
        }
        *process_guard = None;
        *self.stdin_writer.lock().await = None;
        *self.process_model.write().await = None;
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

    async fn set_thinking_level(&self, _level: String) -> Result<(), BackendError> {
        // CLI always has extended thinking enabled; no flag to control it
        Ok(())
    }

    async fn get_messages(&self) -> Vec<Message> {
        self.messages.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use tokio::process::Command;
    use tokio::sync::Mutex;

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
        let backend = ClaudeCliBackend::new();
        assert_eq!(backend.kind(), BackendKind::ClaudeCli);
    }

    #[test]
    fn test_cli_backend_capabilities() {
        let backend = ClaudeCliBackend::new();
        let caps = backend.capabilities();
        assert!(caps.supports_streaming);
        assert!(caps.supports_abort);
        assert!(caps.supports_resume);
        assert!(caps.supports_extended_thinking);
        assert_eq!(caps.permission_model, PermissionModel::PerOperation);
        assert_eq!(caps.available_models.len(), 3);
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

        let message = CliSession::monitor_process_lifecycle(
            process.clone(),
            stdin_writer.clone(),
            abort_flag,
            pid,
        )
        .await;

        assert!(process.lock().await.is_none());
        assert!(stdin_writer.lock().await.is_none());
        assert!(message.is_some());
        assert!(message.unwrap().contains("exited unexpectedly"));
    }

    #[tokio::test]
    async fn monitor_process_lifecycle_suppresses_error_when_aborted() {
        let child = spawn_fast_exit_process(0).await;
        let pid = child.id();
        let process = Arc::new(Mutex::new(Some(child)));
        let stdin_writer = Arc::new(Mutex::new(None));
        let abort_flag = Arc::new(AtomicBool::new(true));

        let message = CliSession::monitor_process_lifecycle(
            process.clone(),
            stdin_writer.clone(),
            abort_flag,
            pid,
        )
        .await;

        assert!(process.lock().await.is_none());
        assert!(stdin_writer.lock().await.is_none());
        assert!(message.is_none());
    }
}
