//! Codex CLI backend adapter.
//!
//! Implements the Backend and BackendSession traits for OpenAI Codex CLI.

use async_trait::async_trait;
use std::path::PathBuf;
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

/// Try to find codex by running a command in a shell
fn try_shell_which(shell: &str, args: &[&str]) -> Option<String> {
    std::process::Command::new(shell)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Get codex version by running the binary directly
fn get_codex_version(codex_path: &str) -> Option<String> {
    std::process::Command::new(codex_path)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .replace("codex-cli ", "")
        })
}

/// Find the Codex CLI path dynamically.
/// Checks common installation paths first, then falls back to shell-based detection.
fn find_codex_path() -> Option<String> {
    // Check common installation paths first (fastest)
    if let Some(home) = dirs::home_dir() {
        let common_paths = [
            PathBuf::from("/opt/homebrew/bin/codex"),
            PathBuf::from("/usr/local/bin/codex"),
            home.join(".local/bin/codex"),
            home.join(".npm-global/bin/codex"),
        ];

        for path in common_paths {
            if path.is_file() {
                let path_str = path.to_string_lossy().to_string();
                // Verify it's actually executable
                if get_codex_version(&path_str).is_some() {
                    return Some(path_str);
                }
            }
        }
    }

    // Determine user's shell
    let user_shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let is_zsh = user_shell.contains("zsh");

    // Try sourcing interactive shell config
    let source_cmd = if is_zsh {
        "source ~/.zshrc 2>/dev/null; which codex"
    } else {
        "source ~/.bashrc 2>/dev/null; which codex"
    };

    if let Some(codex_path) = try_shell_which(&user_shell, &["-c", source_cmd]) {
        return Some(codex_path);
    }

    // Try login shell
    if let Some(codex_path) = try_shell_which(&user_shell, &["-l", "-c", "which codex"]) {
        return Some(codex_path);
    }

    // Final fallback: try both common shells with login flag
    for shell in ["/bin/zsh", "/bin/bash"] {
        if let Some(codex_path) = try_shell_which(shell, &["-l", "-c", "which codex"]) {
            return Some(codex_path);
        }
    }

    None
}

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
            supports_resume: true,
            supports_extended_thinking: true,
            available_models: vec![ModelInfo {
                id: "gpt-5.2-codex".to_string(),
                name: "GPT 5.2 Codex".to_string(),
                supports_thinking: true,
            }],
        }
    }

    async fn check_installed(&self) -> Result<InstallStatus, BackendError> {
        match find_codex_path() {
            Some(path) => {
                let version = get_codex_version(&path);
                Ok(InstallStatus {
                    installed: true,
                    version,
                    path: Some(path),
                })
            }
            None => Ok(InstallStatus {
                installed: false,
                version: None,
                path: None,
            }),
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
        let model = config.model.unwrap_or_else(|| "gpt-5.2".to_string());

        // Find Codex CLI path
        let cli_path = find_codex_path().ok_or_else(|| BackendError {
            message: "Codex CLI not found. Please install it first.".to_string(),
            recoverable: true,
        })?;

        let session = CodexSession::new(
            config.folder_path,
            cli_path,
            permission_mode,
            model,
            app_handle,
        );

        Ok(Arc::new(session))
    }
}

/// Codex session implementing BackendSession.
pub struct CodexSession {
    id: String,
    folder_path: String,
    cli_path: String,
    app_handle: AppHandle,
    child_process: Arc<Mutex<Option<Child>>>,
    abort_flag: Arc<AtomicBool>,
    permission_mode: Arc<RwLock<String>>,
    model: Arc<RwLock<String>>,
    thinking_level: Arc<RwLock<String>>,
    messages: Arc<Mutex<Vec<Message>>>,
    /// Codex's thread ID for multi-turn conversations
    codex_thread_id: Arc<RwLock<Option<String>>>,
}

impl CodexSession {
    pub fn new(
        folder_path: String,
        cli_path: String,
        permission_mode: String,
        model: String,
        app_handle: AppHandle,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            folder_path,
            cli_path,
            app_handle,
            child_process: Arc::new(Mutex::new(None)),
            abort_flag: Arc::new(AtomicBool::new(false)),
            permission_mode: Arc::new(RwLock::new(permission_mode)),
            model: Arc::new(RwLock::new(model)),
            thinking_level: Arc::new(RwLock::new("high".to_string())), // Default reasoning effort
            messages: Arc::new(Mutex::new(Vec::new())),
            codex_thread_id: Arc::new(RwLock::new(None)),
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
        let model = self.model.read().await.clone();
        let thinking_level = self.thinking_level.read().await.clone();

        // Map thinking level to Codex reasoning_effort
        let reasoning_effort = match thinking_level.as_str() {
            "low" => "low",
            "medium" => "medium",
            _ => "high", // default to high
        };

        // Check if we have an existing thread_id for resume
        let existing_thread_id = self.codex_thread_id.read().await.clone();

        // Build command
        let mut cmd = Command::new(&self.cli_path);
        cmd.arg("exec");

        // Use resume subcommand if we have a thread_id from a previous turn
        // Note: resume doesn't support -C or -s flags, so we set current_dir instead
        // We pass -m to ensure the same model is used (Codex CLI warns about model mismatch otherwise)
        if let Some(ref thread_id) = existing_thread_id {
            cmd.arg("resume")
                .arg("--json")
                .arg("--skip-git-repo-check");

            // Pass model to avoid mismatch warning
            if !model.is_empty() {
                cmd.arg("-m").arg(&model);
            }

            // Pass reasoning effort for resume (can be changed mid-conversation)
            cmd.arg("-c")
                .arg(format!("reasoning_effort={}", reasoning_effort));

            cmd.arg(thread_id)
                .arg(message)
                .current_dir(&self.folder_path);
            eprintln!(
                "[codex] Resuming thread {} in {}: codex exec resume --json --skip-git-repo-check -m {} -c reasoning_effort={} {} \"{}\"",
                thread_id, &self.folder_path, model, reasoning_effort, thread_id, message
            );
        } else {
            cmd.arg("--json")
                .arg("--skip-git-repo-check")
                .arg("-C")
                .arg(&self.folder_path)
                .arg("-s")
                .arg(&sandbox);

            // Add model flag if not empty
            if !model.is_empty() {
                cmd.arg("-m").arg(&model);
            }

            // Add reasoning effort config
            cmd.arg("-c").arg(format!("reasoning_effort={}", reasoning_effort));

            cmd.arg(message);
            eprintln!(
                "[codex] Starting new thread: codex exec --json -C {} -s {} -m {} -c reasoning_effort={}",
                &self.folder_path, sandbox, model, reasoning_effort
            );
        }

        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

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
        let codex_thread_id = self.codex_thread_id.clone();

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
                    // Capture thread_id from ThreadStarted event
                    if let CodexEvent::ThreadStarted { ref thread_id } = event {
                        let mut tid = codex_thread_id.write().await;
                        *tid = Some(thread_id.clone());
                        eprintln!("[codex] Captured thread_id: {}", thread_id);
                    }

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

    async fn set_thinking_level(&self, level: String) -> Result<(), BackendError> {
        let mut tl = self.thinking_level.write().await;
        *tl = level;
        Ok(())
    }

    async fn get_messages(&self) -> Vec<Message> {
        self.messages.lock().await.clone()
    }
}
