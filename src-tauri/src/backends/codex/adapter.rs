use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{oneshot, Mutex, Notify, RwLock};
use uuid::Uuid;

use crate::backends::emit_chat_event;
use crate::backends::runtime::PermissionChannels;
use crate::backends::session::BackendSession;
use crate::backends::types::{
    AuthStatus, Backend, BackendCapabilities, BackendError, BackendKind, InstallStatus, ModelInfo,
    PermissionModel, SessionConfig,
};
use crate::commands::chat::{ChatEvent, Message};
use crate::commands::sessions::load_codex_log_messages;
use crate::commands::setup::{
    check_backend_cli_authenticated_internal, check_backend_cli_installed_internal,
};

use super::cli_protocol::{
    clean_thinking_text, event_type, extract_approval_tool_info, first_string,
    normalized_tool_from_item, token_usage_from_turn_completed, final_tool_status,
    IncomingMessage, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse,
};

// ---------------------------------------------------------------------------
// Backend (stateless factory)
// ---------------------------------------------------------------------------

pub struct CodexBackend;

impl CodexBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CodexBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Backend for CodexBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Codex
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
                    id: "gpt-5.3-codex".to_string(),
                    name: "GPT-5.3 Codex".to_string(),
                    supports_thinking: true,
                },
                ModelInfo {
                    id: "gpt-5.2".to_string(),
                    name: "GPT-5.2".to_string(),
                    supports_thinking: true,
                },
                ModelInfo {
                    id: "gpt-5.1-codex-mini".to_string(),
                    name: "GPT-5.1 Codex Mini".to_string(),
                    supports_thinking: false,
                },
            ],
        }
    }

    async fn check_installed(&self) -> Result<InstallStatus, BackendError> {
        let status = check_backend_cli_installed_internal("codex").await;
        Ok(InstallStatus {
            installed: status.installed,
            version: status.version,
            path: status.path,
        })
    }

    async fn check_authenticated(&self) -> Result<AuthStatus, BackendError> {
        let status = check_backend_cli_authenticated_internal("codex").await;
        Ok(AuthStatus {
            authenticated: status.authenticated,
        })
    }

    async fn create_session(
        &self,
        config: SessionConfig,
        app_handle: AppHandle,
    ) -> Result<Arc<dyn BackendSession>, BackendError> {
        Ok(Arc::new(CodexSession::new(config, app_handle).await?))
    }
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

pub struct CodexSession {
    // Identity
    id: String,
    folder_path: String,
    cli_path: Option<String>,
    app_handle: AppHandle,

    // User-controlled settings (per-turn, no process restart needed)
    permission_mode: Arc<RwLock<String>>,
    model: Arc<RwLock<String>>,
    thinking_level: Arc<RwLock<String>>,

    // Long-lived process
    process: Arc<Mutex<Option<Child>>>,
    stdin_writer: Arc<Mutex<Option<tokio::process::ChildStdin>>>,

    // JSON-RPC request/response matching
    next_request_id: AtomicU64,
    pending_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>>,

    // Interactive approvals
    permission_channels: PermissionChannels,

    // Codex state
    thread_id: Arc<RwLock<Option<String>>>,
    codex_turn_id: Arc<RwLock<Option<String>>>,
    initialized: Arc<AtomicBool>,

    // Resume
    resume_session_id: Option<String>,

    // Turn management
    messages: Arc<RwLock<Vec<Message>>>,
    in_flight: Arc<AtomicBool>,
    current_turn_id: Arc<RwLock<Option<String>>>,
    abort_flag: Arc<AtomicBool>,
    abort_notify: Arc<Notify>,
}

impl CodexSession {
    async fn new(config: SessionConfig, app_handle: AppHandle) -> Result<Self, BackendError> {
        let folder_path = config.folder_path.clone();
        let resume_session_id = config.resume_session_id.clone();
        let initial_messages = if let Some(session_id) = resume_session_id.as_deref() {
            load_codex_log_messages(session_id, Some(folder_path.as_str())).unwrap_or_default()
        } else {
            Vec::new()
        };

        let permission_channels: PermissionChannels =
            app_handle.state::<PermissionChannels>().inner().clone();

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            folder_path,
            cli_path: config.cli_path,
            app_handle,
            permission_mode: Arc::new(RwLock::new(
                config
                    .permission_mode
                    .unwrap_or_else(|| "default".to_string()),
            )),
            model: Arc::new(RwLock::new(
                config.model.unwrap_or_else(|| "gpt-5.3-codex".to_string()),
            )),
            thinking_level: Arc::new(RwLock::new("medium".to_string())),
            process: Arc::new(Mutex::new(None)),
            stdin_writer: Arc::new(Mutex::new(None)),
            next_request_id: AtomicU64::new(1),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            permission_channels,
            thread_id: Arc::new(RwLock::new(None)),
            codex_turn_id: Arc::new(RwLock::new(None)),
            initialized: Arc::new(AtomicBool::new(false)),
            resume_session_id,
            messages: Arc::new(RwLock::new(initial_messages)),
            in_flight: Arc::new(AtomicBool::new(false)),
            current_turn_id: Arc::new(RwLock::new(None)),
            abort_flag: Arc::new(AtomicBool::new(false)),
            abort_notify: Arc::new(Notify::new()),
        })
    }

    // -----------------------------------------------------------------------
    // JSON-RPC helpers
    // -----------------------------------------------------------------------

    fn next_id(&self) -> u64 {
        self.next_request_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Write a JSON line to the app-server stdin.
    async fn write_line(&self, value: &impl serde::Serialize) -> Result<(), BackendError> {
        let mut guard = self.stdin_writer.lock().await;
        let writer = guard.as_mut().ok_or_else(|| BackendError {
            message: "Codex app-server stdin not available".to_string(),
            recoverable: false,
        })?;
        let mut line = serde_json::to_string(value).map_err(|e| BackendError {
            message: format!("Failed to serialize JSON-RPC message: {e}"),
            recoverable: false,
        })?;
        line.push('\n');
        writer.write_all(line.as_bytes()).await.map_err(|e| BackendError {
            message: format!("Failed to write to codex stdin: {e}"),
            recoverable: false,
        })?;
        writer.flush().await.map_err(|e| BackendError {
            message: format!("Failed to flush codex stdin: {e}"),
            recoverable: false,
        })?;
        Ok(())
    }

    /// Send a JSON-RPC request and wait for its response.
    async fn send_request(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, BackendError> {
        let id = self.next_id();
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(id, tx);
        }

        let req = JsonRpcRequest::new(method, id, params);
        if let Err(e) = self.write_line(&req).await {
            let mut pending = self.pending_requests.lock().await;
            pending.remove(&id);
            return Err(e);
        }

        let value = rx.await.map_err(|_| BackendError {
            message: format!("No response received for {method} (id={id})"),
            recoverable: false,
        })?;

        // The response router wraps server errors as { "error": ... }.
        if let Some(err) = value.get("error") {
            let msg = err
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_else(|| err.as_str().unwrap_or("unknown error"));
            return Err(BackendError {
                message: format!("{method} failed: {msg}"),
                recoverable: false,
            });
        }

        Ok(value)
    }

    /// Send a JSON-RPC notification (no response expected).
    async fn send_notification(&self, method: &str, params: Value) -> Result<(), BackendError> {
        let notif = JsonRpcNotification::new(method, params);
        self.write_line(&notif).await
    }

    // -----------------------------------------------------------------------
    // Process lifecycle
    // -----------------------------------------------------------------------

    /// Spawn the long-lived `codex app-server` process.
    async fn spawn_app_server(&self) -> Result<(), BackendError> {
        let cli = self
            .cli_path
            .as_deref()
            .unwrap_or("codex");

        let mut command = Command::new(cli);
        command
            .arg("app-server")
            .current_dir(&self.folder_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = command.spawn().map_err(|e| BackendError {
            message: format!("Failed to spawn codex app-server: {e}"),
            recoverable: false,
        })?;

        let stdin = child.stdin.take().ok_or_else(|| BackendError {
            message: "Failed to capture codex app-server stdin".to_string(),
            recoverable: false,
        })?;

        let stdout = child.stdout.take().ok_or_else(|| BackendError {
            message: "Failed to capture codex app-server stdout".to_string(),
            recoverable: false,
        })?;

        let stderr = child.stderr.take().ok_or_else(|| BackendError {
            message: "Failed to capture codex app-server stderr".to_string(),
            recoverable: false,
        })?;

        *self.process.lock().await = Some(child);
        *self.stdin_writer.lock().await = Some(stdin);

        // Spawn stdout reader task
        self.spawn_stdout_reader(stdout);

        // Spawn stderr drain task
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Some(line) = lines.next_line().await.unwrap_or(None) {
                if !line.trim().is_empty() {
                    #[cfg(debug_assertions)]
                    eprintln!("[codex stderr] {}", line.trim());
                }
            }
        });

        // Spawn process monitor
        let process = self.process.clone();
        let stdin_writer = self.stdin_writer.clone();
        let abort_flag = self.abort_flag.clone();
        let in_flight = self.in_flight.clone();
        let app_handle = self.app_handle.clone();
        let session_id = self.id.clone();
        let current_turn_id = self.current_turn_id.clone();
        let initialized = self.initialized.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                let mut guard = process.lock().await;
                let Some(child) = guard.as_mut() else {
                    break;
                };
                match child.try_wait() {
                    Ok(Some(status)) => {
                        *guard = None;
                        drop(guard);
                        *stdin_writer.lock().await = None;
                        initialized.store(false, Ordering::SeqCst);

                        if !abort_flag.load(Ordering::SeqCst) {
                            let turn_id = current_turn_id.read().await.clone();
                            let error_event = ChatEvent::Error {
                                message: format!("Codex app-server exited with status {status}"),
                            };
                            emit_chat_event(
                                &app_handle,
                                Some(&session_id),
                                turn_id.as_deref(),
                                &error_event,
                            );
                            in_flight.store(false, Ordering::SeqCst);
                        }
                        break;
                    }
                    Ok(None) => {} // still running
                    Err(_) => {
                        *guard = None;
                        drop(guard);
                        *stdin_writer.lock().await = None;
                        initialized.store(false, Ordering::SeqCst);
                        break;
                    }
                }
            }
        });

        // Perform handshake
        self.handshake().await?;

        Ok(())
    }

    /// Perform the initialize/initialized handshake.
    async fn handshake(&self) -> Result<(), BackendError> {
        self.send_request(
            "initialize",
            json!({
                "clientInfo": {
                    "name": "caipi",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )
        .await?;

        // Send initialized notification
        self.send_notification("initialized", json!({})).await?;
        self.initialized.store(true, Ordering::SeqCst);

        Ok(())
    }

    /// Ensure we have a thread (start new or resume existing).
    async fn ensure_thread(&self) -> Result<String, BackendError> {
        if let Some(tid) = self.thread_id.read().await.clone() {
            return Ok(tid);
        }

        // If resuming, try thread/resume first
        if let Some(resume_id) = &self.resume_session_id {
            let result = self
                .send_request("thread/resume", json!({ "id": resume_id }))
                .await?;
            if let Some(tid) = result
                .get("threadId")
                .or_else(|| result.get("id"))
                .and_then(Value::as_str)
            {
                let tid = tid.to_string();
                *self.thread_id.write().await = Some(tid.clone());
                return Ok(tid);
            }
            // If resume failed, fall through to create new thread
        }

        let result = self.send_request("thread/start", json!({})).await?;
        let tid = result
            .get("threadId")
            .or_else(|| result.get("id"))
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        *self.thread_id.write().await = Some(tid.clone());

        Ok(tid)
    }

    /// Ensure the app-server is running and initialized.
    async fn ensure_app_server(&self) -> Result<(), BackendError> {
        if self.process.lock().await.is_some() && self.initialized.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.spawn_app_server().await
    }

    // -----------------------------------------------------------------------
    // Permission mode → turn policies
    // -----------------------------------------------------------------------

    fn approval_policy(mode: &str) -> &'static str {
        match mode {
            "bypassPermissions" => "never",
            _ => "unlessTrusted",
        }
    }

    fn sandbox_policy(mode: &str) -> Value {
        match mode {
            "bypassPermissions" => json!({ "type": "dangerFullAccess" }),
            "acceptEdits" => json!({ "type": "workspaceWrite" }),
            _ => json!({ "type": "readOnly" }),
        }
    }

    fn effort_from_thinking(level: &str) -> &str {
        match level {
            "high" => "high",
            "low" => "low",
            _ => "medium",
        }
    }

    // -----------------------------------------------------------------------
    // Stdout reader
    // -----------------------------------------------------------------------

    fn spawn_stdout_reader(&self, stdout: tokio::process::ChildStdout) {
        let pending_requests = self.pending_requests.clone();
        let app_handle = self.app_handle.clone();
        let session_id = self.id.clone();
        let current_turn_id = self.current_turn_id.clone();
        let thread_id = self.thread_id.clone();
        let codex_turn_id = self.codex_turn_id.clone();
        let in_flight = self.in_flight.clone();
        let abort_flag = self.abort_flag.clone();
        let abort_notify = self.abort_notify.clone();
        let permission_mode = self.permission_mode.clone();
        let permission_channels = self.permission_channels.clone();
        let stdin_writer = self.stdin_writer.clone();
        let messages = self.messages.clone();

        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            let mut active_tools: HashMap<String, String> = HashMap::new();
            let mut assistant_parts: Vec<String> = Vec::new();

            while let Some(line) = lines.next_line().await.unwrap_or(None) {
                if line.trim().is_empty() {
                    continue;
                }

                let parsed: Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let turn_id_snapshot = current_turn_id.read().await.clone();
                let turn_id_ref = turn_id_snapshot.as_deref();

                match IncomingMessage::parse(&parsed) {
                    Some(IncomingMessage::Response { id, result, error }) => {
                        // Route response to pending request.
                        // If the server returned an error, wrap it so callers
                        // can distinguish success from failure.
                        let value = if let Some(err) = error {
                            json!({ "error": err })
                        } else {
                            result.unwrap_or(Value::Null)
                        };
                        let sender = {
                            let mut pending = pending_requests.lock().await;
                            pending.remove(&id)
                        };
                        if let Some(tx) = sender {
                            let _ = tx.send(value);
                        }
                    }

                    Some(IncomingMessage::Notification { method, params }) => {
                        Self::handle_notification(
                            &method,
                            &params,
                            &app_handle,
                            &session_id,
                            turn_id_ref,
                            &thread_id,
                            &codex_turn_id,
                            &in_flight,
                            &messages,
                            &mut active_tools,
                            &mut assistant_parts,
                        )
                        .await;
                    }

                    Some(IncomingMessage::Request { id, method, params }) => {
                        Self::handle_approval_request(
                            id,
                            &method,
                            &params,
                            &app_handle,
                            &session_id,
                            turn_id_ref,
                            &permission_mode,
                            &permission_channels,
                            &stdin_writer,
                            &abort_flag,
                            &abort_notify,
                            &mut active_tools,
                        )
                        .await;
                    }

                    None => {
                        // Possibly a legacy-format event line — try to handle as
                        // notification using the "type" field (backwards compat with
                        // older Codex versions that may mix formats).
                        if let Some(kind) = event_type(&parsed) {
                            Self::handle_legacy_event(
                                kind,
                                &parsed,
                                &app_handle,
                                &session_id,
                                turn_id_ref,
                                &thread_id,
                                &codex_turn_id,
                                &in_flight,
                                &messages,
                                &mut active_tools,
                                &mut assistant_parts,
                            )
                            .await;
                        }
                    }
                }
            }
        });
    }

    /// Handle a JSON-RPC notification from the app-server.
    #[allow(clippy::too_many_arguments)]
    async fn handle_notification(
        method: &str,
        params: &Value,
        app_handle: &AppHandle,
        session_id: &str,
        turn_id: Option<&str>,
        thread_id: &Arc<RwLock<Option<String>>>,
        codex_turn_id: &Arc<RwLock<Option<String>>>,
        in_flight: &AtomicBool,
        messages: &Arc<RwLock<Vec<Message>>>,
        active_tools: &mut HashMap<String, String>,
        assistant_parts: &mut Vec<String>,
    ) {
        match method {
            "thread/started" => {
                if let Some(tid) = params
                    .get("threadId")
                    .or_else(|| params.get("id"))
                    .and_then(Value::as_str)
                {
                    *thread_id.write().await = Some(tid.to_string());
                }
            }

            "turn/started" => {
                if let Some(tid) = params
                    .get("turnId")
                    .or_else(|| params.get("id"))
                    .and_then(Value::as_str)
                {
                    *codex_turn_id.write().await = Some(tid.to_string());
                }
                // Clear accumulation for new turn
                assistant_parts.clear();
                active_tools.clear();
            }

            "item/started" => {
                Self::handle_item_started(params, app_handle, session_id, turn_id, active_tools);
            }

            "item/agentMessage/delta" | "item/delta" => {
                if let Some(text) = params
                    .get("delta")
                    .or_else(|| params.get("text"))
                    .and_then(Value::as_str)
                {
                    if !text.is_empty() {
                        assistant_parts.push(text.to_string());
                        let event = ChatEvent::Text {
                            content: text.to_string(),
                        };
                        emit_chat_event(app_handle, Some(session_id), turn_id, &event);
                    }
                }
            }

            "item/completed" => {
                Self::handle_item_completed(
                    params,
                    app_handle,
                    session_id,
                    turn_id,
                    active_tools,
                    assistant_parts,
                );
            }

            "turn/completed" => {
                // Emit token usage if available
                if let Some((total, ctx, window)) = token_usage_from_turn_completed(params) {
                    let usage_event = ChatEvent::TokenUsage {
                        total_tokens: total,
                        context_tokens: ctx,
                        context_window: window,
                    };
                    emit_chat_event(app_handle, Some(session_id), turn_id, &usage_event);
                }

                // Store assistant message
                let text = assistant_parts.join("");
                if !text.trim().is_empty() {
                    let mut msgs = messages.write().await;
                    msgs.push(Message {
                        id: Uuid::new_v4().to_string(),
                        role: "assistant".to_string(),
                        content: text,
                        timestamp: chrono::Utc::now().timestamp(),
                    });
                }
                assistant_parts.clear();

                // Clear codex turn id
                *codex_turn_id.write().await = None;

                // Emit completion
                let complete_event = ChatEvent::Complete;
                emit_chat_event(app_handle, Some(session_id), turn_id, &complete_event);
                in_flight.store(false, Ordering::SeqCst);
            }

            _ => {
                // Unknown notification — ignore
            }
        }
    }

    fn handle_item_started(
        params: &Value,
        app_handle: &AppHandle,
        session_id: &str,
        turn_id: Option<&str>,
        active_tools: &mut HashMap<String, String>,
    ) {
        let item = params.get("item").unwrap_or(params);
        let item_kind = first_string(item, &[&["type"], &["kind"]])
            .or_else(|| first_string(params, &[&["item_type"], &["kind"]]))
            .unwrap_or("tool");
        let item_id = first_string(item, &[&["id"]])
            .or_else(|| first_string(params, &[&["item_id"], &["id"]]))
            .unwrap_or("item")
            .to_string();

        if item_kind.contains("reason") {
            let thinking_content =
                clean_thinking_text(first_string(item, &[&["text"]]).unwrap_or("Thinking"));
            let event = ChatEvent::ThinkingStart {
                thinking_id: item_id,
                content: thinking_content,
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &event);
        } else if item_kind == "agent_message" {
            // Text messages handled via delta/completed
        } else {
            let (tool_type, target, input) = normalized_tool_from_item(item);
            active_tools.insert(item_id.clone(), tool_type.clone());
            let event = ChatEvent::ToolStart {
                tool_use_id: item_id.clone(),
                tool_type,
                target,
                status: "running".to_string(),
                input,
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &event);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_item_completed(
        params: &Value,
        app_handle: &AppHandle,
        session_id: &str,
        turn_id: Option<&str>,
        active_tools: &mut HashMap<String, String>,
        assistant_parts: &mut Vec<String>,
    ) {
        let item = params.get("item").unwrap_or(params);
        let item_kind = first_string(item, &[&["type"], &["kind"]])
            .or_else(|| first_string(params, &[&["item_type"], &["kind"]]))
            .unwrap_or("tool");
        let item_id = first_string(item, &[&["id"]])
            .or_else(|| first_string(params, &[&["item_id"], &["id"]]))
            .unwrap_or("item")
            .to_string();

        if item_kind.contains("reason") {
            let thinking_content =
                clean_thinking_text(first_string(item, &[&["text"]]).unwrap_or("Thinking"));
            // Emit start+end for reasoning blocks
            let start = ChatEvent::ThinkingStart {
                thinking_id: item_id.clone(),
                content: thinking_content,
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &start);
            let end = ChatEvent::ThinkingEnd {
                thinking_id: item_id,
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &end);
        } else if item_kind == "agent_message" {
            // Emit full text if we haven't received deltas
            if let Some(text) = first_string(item, &[&["text"], &["content", "text"]]) {
                if !text.is_empty() {
                    assistant_parts.push(text.to_string());
                    let event = ChatEvent::Text {
                        content: text.to_string(),
                    };
                    emit_chat_event(app_handle, Some(session_id), turn_id, &event);
                }
            }
        } else if item_kind == "web_search_call" && !active_tools.contains_key(&item_id) {
            let target = first_string(item, &[&["action", "query"], &["query"]])
                .unwrap_or("")
                .to_string();
            let start = ChatEvent::ToolStart {
                tool_use_id: item_id.clone(),
                tool_type: "web_search".to_string(),
                target,
                status: "pending".to_string(),
                input: None,
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &start);
            let end = ChatEvent::ToolEnd {
                id: item_id,
                status: "completed".to_string(),
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &end);
        } else if item_kind == "file_change" && !active_tools.contains_key(&item_id) {
            let target = item
                .get("changes")
                .and_then(Value::as_array)
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("path"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let start = ChatEvent::ToolStart {
                tool_use_id: item_id.clone(),
                tool_type: "file_change".to_string(),
                target,
                status: "pending".to_string(),
                input: None,
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &start);
            let end = ChatEvent::ToolEnd {
                id: item_id,
                status: "completed".to_string(),
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &end);
        } else if active_tools.contains_key(&item_id) {
            let tool_type = active_tools
                .remove(&item_id)
                .unwrap_or_else(|| item_kind.to_string());
            let completed_status = first_string(item, &[&["status"]]).unwrap_or("completed");
            let exit_code = item.get("exit_code").and_then(Value::as_i64);
            let status = final_tool_status(&tool_type, completed_status, exit_code);
            let end = ChatEvent::ToolEnd {
                id: item_id,
                status: status.to_string(),
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &end);
        }
    }

    /// Handle legacy-format events (type-based rather than JSON-RPC method-based).
    /// This provides backwards compatibility with older Codex versions.
    #[allow(clippy::too_many_arguments)]
    async fn handle_legacy_event(
        kind: &str,
        parsed: &Value,
        app_handle: &AppHandle,
        session_id: &str,
        turn_id: Option<&str>,
        thread_id: &Arc<RwLock<Option<String>>>,
        codex_turn_id: &Arc<RwLock<Option<String>>>,
        in_flight: &AtomicBool,
        messages: &Arc<RwLock<Vec<Message>>>,
        active_tools: &mut HashMap<String, String>,
        assistant_parts: &mut Vec<String>,
    ) {
        // Map legacy event types to notification methods
        let (method, params) = match kind {
            "thread.started" => ("thread/started", parsed.clone()),
            "turn.started" => ("turn/started", parsed.clone()),
            "item.started" => ("item/started", parsed.clone()),
            "item.completed" => ("item/completed", parsed.clone()),
            "turn.completed" => ("turn/completed", parsed.clone()),
            "error" => {
                if let Some(err) =
                    first_string(parsed, &[&["message"], &["error"], &["error", "message"]])
                {
                    let event = ChatEvent::Error {
                        message: err.to_string(),
                    };
                    emit_chat_event(app_handle, Some(session_id), turn_id, &event);
                }
                return;
            }
            _ => {
                // Try to extract text from unknown events
                let item_kind = first_string(parsed, &[&["item", "type"], &["item_type"], &["kind"]])
                    .unwrap_or("");
                let should_emit_text = !(item_kind.contains("reason")
                    || item_kind == "command_execution"
                    || item_kind == "function_call"
                    || item_kind == "web_search"
                    || item_kind == "web_search_call"
                    || item_kind == "file_change");

                if should_emit_text {
                    if let Some(text) = first_string(
                        parsed,
                        &[
                            &["delta"],
                            &["text"],
                            &["content"],
                            &["item", "text"],
                            &["item", "content", "text"],
                            &["message", "content", "text"],
                        ],
                    ) {
                        if !text.is_empty() {
                            assistant_parts.push(text.to_string());
                            let event = ChatEvent::Text {
                                content: text.to_string(),
                            };
                            emit_chat_event(app_handle, Some(session_id), turn_id, &event);
                        }
                    }
                }
                return;
            }
        };

        Self::handle_notification(
            method,
            &params,
            app_handle,
            session_id,
            turn_id,
            thread_id,
            codex_turn_id,
            in_flight,
            messages,
            active_tools,
            assistant_parts,
        )
        .await;
    }

    /// Handle an incoming approval request from the server.
    #[allow(clippy::too_many_arguments)]
    async fn handle_approval_request(
        request_id: Value,
        method: &str,
        params: &Value,
        app_handle: &AppHandle,
        session_id: &str,
        turn_id: Option<&str>,
        permission_mode: &Arc<RwLock<String>>,
        permission_channels: &PermissionChannels,
        stdin_writer: &Arc<Mutex<Option<tokio::process::ChildStdin>>>,
        abort_flag: &Arc<AtomicBool>,
        abort_notify: &Arc<Notify>,
        active_tools: &mut HashMap<String, String>,
    ) {
        let (tool_type, target) = extract_approval_tool_info(method, params);
        let tool_use_id = Uuid::new_v4().to_string();

        // Emit ToolStart with pending status
        active_tools.insert(tool_use_id.clone(), tool_type.clone());
        let start_event = ChatEvent::ToolStart {
            tool_use_id: tool_use_id.clone(),
            tool_type: tool_type.clone(),
            target: target.clone(),
            status: "pending".to_string(),
            input: None,
        };
        emit_chat_event(app_handle, Some(session_id), turn_id, &start_event);

        let mode = permission_mode.read().await.clone();

        // Decide whether to auto-accept or prompt user
        let auto_accept = match mode.as_str() {
            "bypassPermissions" => true,
            "acceptEdits" if tool_type == "file_change" => true,
            _ => false,
        };

        let allowed = if auto_accept {
            // Auto-accept: update status to running
            let running_event = ChatEvent::ToolStatusUpdate {
                tool_use_id: tool_use_id.clone(),
                status: "running".to_string(),
                permission_request_id: None,
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &running_event);
            true
        } else {
            // Prompt user: emit awaiting_permission and wait
            let permission_request_id = Uuid::new_v4().to_string();
            let (tx, rx) = oneshot::channel();
            {
                let mut channels = permission_channels.lock().await;
                channels.insert(permission_request_id.clone(), tx);
            }

            let awaiting_event = ChatEvent::ToolStatusUpdate {
                tool_use_id: tool_use_id.clone(),
                status: "awaiting_permission".to_string(),
                permission_request_id: Some(permission_request_id.clone()),
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &awaiting_event);

            // Wait for user decision, timeout, or abort
            let decision = tokio::select! {
                resp = rx => {
                    resp.map(|r| r.allowed).unwrap_or(false)
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {
                    false
                }
                _ = abort_notify.notified() => {
                    false
                }
            };

            // Cleanup channel
            {
                let mut channels = permission_channels.lock().await;
                channels.remove(&permission_request_id);
            }

            // Emit status update
            let status = if decision { "running" } else { "denied" };
            let status_event = ChatEvent::ToolStatusUpdate {
                tool_use_id: tool_use_id.clone(),
                status: status.to_string(),
                permission_request_id: None,
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &status_event);

            decision
        };

        // Send the approval response to the server
        let decision_str = if allowed { "accept" } else { "decline" };
        let response = JsonRpcResponse::new(
            request_id,
            json!({ "decision": decision_str }),
        );

        let mut line = match serde_json::to_string(&response) {
            Ok(l) => l,
            Err(_) => return,
        };
        line.push('\n');

        let mut guard = stdin_writer.lock().await;
        if let Some(writer) = guard.as_mut() {
            let _ = writer.write_all(line.as_bytes()).await;
            let _ = writer.flush().await;
        }

        // If denied and abort was requested, remove from active tools
        if !allowed && abort_flag.load(Ordering::SeqCst) {
            active_tools.remove(&tool_use_id);
        }
    }
}

// ---------------------------------------------------------------------------
// BackendSession implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl BackendSession for CodexSession {
    fn session_id(&self) -> &str {
        &self.id
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::Codex
    }

    fn folder_path(&self) -> &str {
        &self.folder_path
    }

    async fn send_message(&self, message: &str, turn_id: Option<&str>) -> Result<(), BackendError> {
        if self.in_flight.swap(true, Ordering::SeqCst) {
            return Err(BackendError {
                message: "Codex session is busy".to_string(),
                recoverable: true,
            });
        }

        let turn_id = turn_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        *self.current_turn_id.write().await = Some(turn_id.clone());
        self.abort_flag.store(false, Ordering::SeqCst);

        // Store user message
        {
            let mut messages = self.messages.write().await;
            messages.push(Message {
                id: Uuid::new_v4().to_string(),
                role: "user".to_string(),
                content: message.to_string(),
                timestamp: chrono::Utc::now().timestamp(),
            });
        }

        // Ensure app-server is running
        if let Err(e) = self.ensure_app_server().await {
            self.in_flight.store(false, Ordering::SeqCst);
            *self.current_turn_id.write().await = None;
            return Err(e);
        }

        // Ensure thread exists
        let thread_id = match self.ensure_thread().await {
            Ok(tid) => tid,
            Err(e) => {
                self.in_flight.store(false, Ordering::SeqCst);
                *self.current_turn_id.write().await = None;
                return Err(e);
            }
        };

        // Emit session init
        let session_init = ChatEvent::SessionInit {
            auth_type: "codex".to_string(),
        };
        emit_chat_event(
            &self.app_handle,
            Some(&self.id),
            Some(&turn_id),
            &session_init,
        );

        // Build turn/start params
        let current_model = self.model.read().await.clone();
        let current_thinking = self.thinking_level.read().await.clone();
        let mode = self.permission_mode.read().await.clone();

        let params = json!({
            "threadId": thread_id,
            "input": [{ "type": "text", "text": message }],
            "model": current_model,
            "effort": Self::effort_from_thinking(&current_thinking),
            "approvalPolicy": Self::approval_policy(&mode),
            "sandboxPolicy": Self::sandbox_policy(&mode),
        });

        // Register pending request *before* writing so a fast reply isn't dropped.
        // We don't await the response — turn results stream via notifications.
        let id = self.next_id();
        let (tx, _rx) = oneshot::channel();
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(id, tx);
        }

        let req = JsonRpcRequest::new("turn/start", id, params);
        if let Err(e) = self.write_line(&req).await {
            self.pending_requests.lock().await.remove(&id);
            self.in_flight.store(false, Ordering::SeqCst);
            *self.current_turn_id.write().await = None;
            return Err(e);
        }

        Ok(())
    }

    async fn abort(&self) -> Result<(), BackendError> {
        let active_turn = self.current_turn_id.read().await.clone();

        self.abort_flag.store(true, Ordering::SeqCst);
        self.abort_notify.notify_waiters();

        // Try graceful interrupt via turn/interrupt
        let thread = self.thread_id.read().await.clone();
        let codex_turn = self.codex_turn_id.read().await.clone();
        if let (Some(tid), Some(ctid)) = (thread, codex_turn) {
            let _ = self
                .send_notification(
                    "turn/interrupt",
                    json!({ "threadId": tid, "turnId": ctid }),
                )
                .await;
        }

        // Wait briefly for graceful completion
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Kill process if still running
        {
            let mut guard = self.process.lock().await;
            if let Some(child) = guard.as_mut() {
                let _ = child.kill().await;
            }
            *guard = None;
        }
        *self.stdin_writer.lock().await = None;
        self.initialized.store(false, Ordering::SeqCst);

        // Release state
        self.in_flight.store(false, Ordering::SeqCst);
        *self.current_turn_id.write().await = None;
        *self.codex_turn_id.write().await = None;

        // Clear pending requests
        {
            let mut pending = self.pending_requests.lock().await;
            pending.clear();
        }

        let abort_complete = ChatEvent::AbortComplete {
            session_id: self.id.clone(),
        };
        emit_chat_event(
            &self.app_handle,
            Some(&self.id),
            active_turn.as_deref(),
            &abort_complete,
        );

        Ok(())
    }

    async fn cleanup(&self) {
        let _ = self.abort().await;
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

    async fn get_messages(&self) -> Vec<Message> {
        self.messages.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        clean_thinking_text, final_tool_status, normalized_tool_from_item, CodexSession,
    };
    use serde_json::json;

    #[test]
    fn clean_thinking_text_strips_wrapping_bold_markers() {
        assert_eq!(
            clean_thinking_text("**Planning command execution updates**"),
            "Planning command execution updates"
        );
    }

    #[test]
    fn clean_thinking_text_keeps_normal_text() {
        assert_eq!(clean_thinking_text("Thinking..."), "Thinking...");
    }

    #[test]
    fn final_tool_status_for_command_requires_zero_exit_code() {
        assert_eq!(final_tool_status("command_execution", "completed", Some(0)), "completed");
        assert_eq!(final_tool_status("command_execution", "completed", Some(1)), "error");
        assert_eq!(final_tool_status("command_execution", "completed", None), "error");
    }

    #[test]
    fn final_tool_status_for_non_command_uses_item_status() {
        assert_eq!(final_tool_status("web_search", "completed", None), "completed");
        assert_eq!(final_tool_status("web_search", "failed", None), "error");
    }

    #[test]
    fn normalized_tool_from_item_maps_web_run_search_to_web_search() {
        let item = json!({
            "type": "function_call",
            "name": "web.run",
            "arguments": "{\"search_query\":[{\"q\":\"latest rust release\"}]}"
        });
        let (tool_type, target, input) = normalized_tool_from_item(&item);
        assert_eq!(tool_type, "web_search");
        assert_eq!(target, "latest rust release");
        assert!(input.is_some());
    }

    #[test]
    fn normalized_tool_from_item_maps_web_run_open_to_web_fetch() {
        let item = json!({
            "type": "function_call",
            "name": "web.run",
            "arguments": "{\"open\":[{\"ref_id\":\"turn0search0\"}]}"
        });
        let (tool_type, target, input) = normalized_tool_from_item(&item);
        assert_eq!(tool_type, "web_fetch");
        assert_eq!(target, "turn0search0");
        assert!(input.is_some());
    }

    #[test]
    fn approval_policy_mapping() {
        assert_eq!(CodexSession::approval_policy("default"), "unlessTrusted");
        assert_eq!(CodexSession::approval_policy("acceptEdits"), "unlessTrusted");
        assert_eq!(CodexSession::approval_policy("bypassPermissions"), "never");
    }

    #[test]
    fn sandbox_policy_mapping() {
        let default = CodexSession::sandbox_policy("default");
        assert_eq!(default.get("type").and_then(|v| v.as_str()), Some("readOnly"));

        let edit = CodexSession::sandbox_policy("acceptEdits");
        assert_eq!(edit.get("type").and_then(|v| v.as_str()), Some("workspaceWrite"));

        let bypass = CodexSession::sandbox_policy("bypassPermissions");
        assert_eq!(bypass.get("type").and_then(|v| v.as_str()), Some("dangerFullAccess"));
    }

    #[test]
    fn effort_mapping() {
        assert_eq!(CodexSession::effort_from_thinking("high"), "high");
        assert_eq!(CodexSession::effort_from_thinking("low"), "low");
        assert_eq!(CodexSession::effort_from_thinking("medium"), "medium");
        assert_eq!(CodexSession::effort_from_thinking(""), "medium");
    }
}
