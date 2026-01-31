//! Codex CLI backend adapter.
//!
//! Implements the Backend and BackendSession traits for OpenAI Codex CLI.

use async_trait::async_trait;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, RwLock};

use crate::backends::session::BackendSession;
use crate::backends::types::{
    AuthStatus, Backend, BackendCapabilities, BackendError, BackendKind, InstallStatus, ModelInfo,
    PermissionModel, SessionConfig,
};
use crate::commands::chat::{ChatEvent, Message};

use super::events::{translate_event, CodexEvent};

const CODEX_PATH: &str = "/opt/homebrew/bin/codex";

/// Codex CLI backend implementation.
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
            permission_model: PermissionModel::SessionLevel,
            supports_streaming: true,
            supports_abort: true,
            supports_resume: false,
            supports_extended_thinking: true,
            available_models: vec![ModelInfo {
                id: "gpt-5.2-codex".to_string(),
                name: "GPT 5.2 Codex".to_string(),
                supports_thinking: true,
            }],
        }
    }

    async fn check_installed(&self) -> Result<InstallStatus, BackendError> {
        let output = Command::new(CODEX_PATH)
            .arg("--version")
            .output()
            .await
            .map_err(|e| BackendError {
                message: format!("Failed to check Codex installation: {}", e),
                recoverable: true,
            })?;

        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout)
                .trim()
                .replace("codex-cli ", "");
            Ok(InstallStatus {
                installed: true,
                version: Some(version),
                path: Some(CODEX_PATH.to_string()),
            })
        } else {
            Ok(InstallStatus {
                installed: false,
                version: None,
                path: None,
            })
        }
    }

    async fn check_authenticated(&self) -> Result<AuthStatus, BackendError> {
        // Codex doesn't have a direct "auth check" command, but we can verify
        // by checking if config exists or by trying a simple exec
        // For now, assume authenticated if installed (user logged in via ChatGPT)
        let install_status = self.check_installed().await?;
        Ok(AuthStatus {
            authenticated: install_status.installed,
        })
    }

    async fn create_session(
        &self,
        config: SessionConfig,
        app_handle: AppHandle,
    ) -> Result<Arc<dyn BackendSession>, BackendError> {
        let permission_mode = config.permission_mode.unwrap_or_else(|| "default".to_string());

        let session = CodexSession::new(
            config.folder_path,
            permission_mode,
            app_handle,
        );

        Ok(Arc::new(session))
    }
}

/// Codex session implementing BackendSession.
pub struct CodexSession {
    id: String,
    folder_path: String,
    app_handle: AppHandle,
    child_process: Arc<Mutex<Option<Child>>>,
    abort_flag: Arc<AtomicBool>,
    permission_mode: Arc<RwLock<String>>,
    model: Arc<RwLock<String>>,
    messages: Arc<Mutex<Vec<Message>>>,
}

impl CodexSession {
    pub fn new(
        folder_path: String,
        permission_mode: String,
        app_handle: AppHandle,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            folder_path,
            app_handle,
            child_process: Arc::new(Mutex::new(None)),
            abort_flag: Arc::new(AtomicBool::new(false)),
            permission_mode: Arc::new(RwLock::new(permission_mode)),
            model: Arc::new(RwLock::new(String::new())), // Not used for Codex
            messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Maps Caipi permission mode to Codex sandbox flag.
    fn get_sandbox_mode(&self, mode: &str) -> String {
        match mode {
            "acceptEdits" => "workspace-write".to_string(),
            "bypassPermissions" => "danger-full-access".to_string(),
            _ => "read-only".to_string(), // "default"
        }
    }
}

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

    async fn send_message(&self, message: &str) -> Result<(), BackendError> {
        // Reset abort flag
        self.abort_flag.store(false, Ordering::SeqCst);

        // Store user message
        {
            let mut msgs = self.messages.lock().await;
            msgs.push(Message {
                id: uuid::Uuid::new_v4().to_string(),
                role: "user".to_string(),
                content: message.to_string(),
                timestamp: chrono::Utc::now().timestamp_millis(),
            });
        }

        let permission_mode = self.permission_mode.read().await.clone();
        let sandbox = self.get_sandbox_mode(&permission_mode);

        // Build command (let Codex use its default model)
        let mut cmd = Command::new(CODEX_PATH);
        cmd.arg("exec")
            .arg("--json")
            .arg("--skip-git-repo-check")
            .arg("-C")
            .arg(&self.folder_path)
            .arg("-s")
            .arg(&sandbox)
            .arg(message)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        eprintln!(
            "[codex] Spawning: codex exec --json -C {} -s {}",
            &self.folder_path, sandbox
        );

        let mut child = cmd.spawn().map_err(|e| BackendError {
            message: format!("Failed to spawn Codex process: {}", e),
            recoverable: false,
        })?;

        let stdout = child.stdout.take().ok_or_else(|| BackendError {
            message: "Failed to capture stdout".to_string(),
            recoverable: false,
        })?;

        // Store child process for abort
        {
            let mut proc = self.child_process.lock().await;
            *proc = Some(child);
        }

        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        let app_handle = self.app_handle.clone();
        let abort_flag = self.abort_flag.clone();
        let messages = self.messages.clone();

        // Accumulate assistant response
        let mut assistant_content = String::new();

        // Process JSONL output
        while let Ok(Some(line)) = lines.next_line().await {
            // Check abort flag
            if abort_flag.load(Ordering::SeqCst) {
                eprintln!("[codex] Abort flag detected, breaking read loop");
                break;
            }

            if line.trim().is_empty() {
                continue;
            }

            eprintln!("[codex] JSONL: {}", line);

            // Parse the event
            match serde_json::from_str::<CodexEvent>(&line) {
                Ok(event) => {
                    let chat_events = translate_event(&event);
                    for chat_event in chat_events {
                        // Accumulate text for message history
                        if let ChatEvent::Text { ref content } = chat_event {
                            assistant_content.push_str(content);
                        }
                        let _ = app_handle.emit("claude:event", &chat_event);
                    }
                }
                Err(e) => {
                    eprintln!("[codex] Failed to parse JSONL: {} - line: {}", e, line);
                }
            }
        }

        // Store assistant message if we got any content
        if !assistant_content.is_empty() {
            let mut msgs = messages.lock().await;
            msgs.push(Message {
                id: uuid::Uuid::new_v4().to_string(),
                role: "assistant".to_string(),
                content: assistant_content,
                timestamp: chrono::Utc::now().timestamp_millis(),
            });
        }

        // Clean up child process
        {
            let mut proc = self.child_process.lock().await;
            if let Some(mut child) = proc.take() {
                let _ = child.wait().await;
            }
        }

        Ok(())
    }

    async fn abort(&self) -> Result<(), BackendError> {
        eprintln!("[codex] Aborting session {}", self.id);

        // Set abort flag
        self.abort_flag.store(true, Ordering::SeqCst);

        // Kill child process if running
        let mut proc = self.child_process.lock().await;
        if let Some(ref mut child) = *proc {
            let _ = child.kill().await;
            eprintln!("[codex] Killed child process");
        }

        // Emit abort complete
        let _ = self.app_handle.emit(
            "claude:event",
            &ChatEvent::AbortComplete {
                session_id: self.id.clone(),
            },
        );

        Ok(())
    }

    async fn cleanup(&self) {
        eprintln!("[codex] Cleaning up session {}", self.id);
        let _ = self.abort().await;
    }

    async fn get_permission_mode(&self) -> String {
        self.permission_mode.read().await.clone()
    }

    async fn set_permission_mode(&self, mode: String) -> Result<(), BackendError> {
        let mut pm = self.permission_mode.write().await;
        *pm = mode;
        Ok(())
    }

    async fn get_model(&self) -> String {
        self.model.read().await.clone()
    }

    async fn set_model(&self, model: String) -> Result<(), BackendError> {
        let mut m = self.model.write().await;
        *m = model;
        Ok(())
    }

    async fn set_extended_thinking(&self, _enabled: bool) -> Result<(), BackendError> {
        // Codex uses reasoning automatically based on model config
        // We could potentially use -c reasoning_effort=... but for now no-op
        Ok(())
    }

    async fn get_messages(&self) -> Vec<Message> {
        self.messages.lock().await.clone()
    }
}
