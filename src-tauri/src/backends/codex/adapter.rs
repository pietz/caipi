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
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::backends::emit_chat_event;
use crate::backends::runtime::PermissionChannels;
use crate::backends::session::BackendSession;
use crate::backends::types::{
    AuthStatus, Backend, BackendError, BackendKind, InstallStatus, SessionConfig,
};
use crate::backends::types::{ChatEvent, Message};
use super::sessions::load_codex_log_messages;
use crate::commands::setup::{
    check_backend_cli_authenticated_internal, check_backend_cli_installed_internal,
};

use super::cli_protocol::{
    event_type, IncomingMessage, JsonRpcNotification, JsonRpcRequest,
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
    // Background task handles for the active process.
    stdout_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    stderr_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    monitor_task: Arc<Mutex<Option<JoinHandle<()>>>>,
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
            stdout_task: Arc::new(Mutex::new(None)),
            stderr_task: Arc::new(Mutex::new(None)),
            monitor_task: Arc::new(Mutex::new(None)),
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

        let value = rx.await.map_err(|_| {
            log::error!("RPC timeout: method={}, id={}", method, id);
            BackendError {
                message: format!("No response received for {method} (id={id})"),
                recoverable: false,
            }
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

    async fn abort_background_tasks(&self) {
        crate::backends::utils::abort_task_slot(&self.stdout_task).await;
        crate::backends::utils::abort_task_slot(&self.stderr_task).await;
        crate::backends::utils::abort_task_slot(&self.monitor_task).await;
    }

    async fn stop_process(&self) {
        self.abort_background_tasks().await;

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
    }

    // -----------------------------------------------------------------------
    // Process lifecycle
    // -----------------------------------------------------------------------

    /// Spawn the long-lived `codex app-server` process.
    async fn spawn_app_server(&self) -> Result<(), BackendError> {
        // Defensive: ensure stale reader/monitor tasks are gone before spawning again.
        self.abort_background_tasks().await;

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
            command.creation_flags(crate::backends::utils::CREATE_NO_WINDOW);
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

        log::info!("Codex app-server spawned: folder={}", self.folder_path);

        // Spawn stdout reader task
        let stdout_handle = self.spawn_stdout_reader(stdout);
        *self.stdout_task.lock().await = Some(stdout_handle);

        // Spawn stderr drain task
        let stderr_handle = crate::backends::utils::spawn_stderr_drain(stderr, "codex");
        *self.stderr_task.lock().await = Some(stderr_handle);

        // Spawn process monitor
        let process = self.process.clone();
        let stdin_writer = self.stdin_writer.clone();
        let abort_flag = self.abort_flag.clone();
        let in_flight = self.in_flight.clone();
        let app_handle = self.app_handle.clone();
        let session_id = self.id.clone();
        let current_turn_id = self.current_turn_id.clone();
        let initialized = self.initialized.clone();
        let pending_requests = self.pending_requests.clone();
        let monitor_handle = tokio::spawn(async move {
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
                        // Clear pending requests so any awaiting send_request callers
                        // get a RecvError instead of hanging forever
                        pending_requests.lock().await.clear();

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
                    Ok(None) => {
                        drop(guard);
                    }
                    Err(_) => {
                        *guard = None;
                        drop(guard);
                        *stdin_writer.lock().await = None;
                        initialized.store(false, Ordering::SeqCst);
                        pending_requests.lock().await.clear();
                        break;
                    }
                }
            }
        });
        *self.monitor_task.lock().await = Some(monitor_handle);

        // Perform handshake
        if let Err(e) = self.handshake().await {
            self.stop_process().await;
            return Err(e);
        }

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
        log::info!("Codex handshake complete, session initialized");

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
                .send_request("thread/resume", json!({ "threadId": resume_id }))
                .await?;
            if let Some(tid) = result
                .pointer("/thread/id")
                .or_else(|| result.get("threadId"))
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
            .or_else(|| result.pointer("/thread/id"))
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
    // Permission mode -> turn policies
    // -----------------------------------------------------------------------

    fn approval_policy(mode: &str) -> &'static str {
        match mode {
            "bypassPermissions" => "never",
            _ => "on-request",
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

    fn spawn_stdout_reader(&self, stdout: tokio::process::ChildStdout) -> JoinHandle<()> {
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
                    Err(e) => {
                        log::warn!("Codex JSON parse error: {e}");
                        continue;
                    }
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
                        // Possibly a legacy-format event line -- try to handle as
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
        })
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
            messages.push(Message::new("user", message));
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

        let id = self.next_id();
        let req = JsonRpcRequest::new("turn/start", id, params);
        if let Err(e) = self.write_line(&req).await {
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

        // Try graceful interrupt via turn/interrupt (fire-and-forget to avoid
        // hanging if the server is unresponsive or already dead)
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

        self.stop_process().await;

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
        // Keep shutdown silent on app-close/session-destroy cleanup.
        self.abort_flag.store(true, Ordering::SeqCst);
        self.abort_notify.notify_waiters();
        self.stop_process().await;
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
    use super::super::cli_protocol::{
        clean_thinking_text, final_tool_status, normalized_tool_from_item,
    };
    use super::CodexSession;
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
        assert_eq!(CodexSession::approval_policy("default"), "on-request");
        assert_eq!(CodexSession::approval_policy("acceptEdits"), "on-request");
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
