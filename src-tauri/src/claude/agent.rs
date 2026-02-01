use crate::commands::chat::{Message as ChatMessage, ChatEvent};
use claude_agent_sdk_rs::{
    ClaudeClient, ClaudeAgentOptions, Message, ContentBlock,
    PermissionMode, SettingSource,
};
use futures::StreamExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};
use thiserror::Error;
use tokio::sync::{Mutex, Notify, RwLock};
use uuid::Uuid;

use super::hooks::build_hooks;

// Re-export from hooks for external use
pub use super::hooks::PermissionChannels;

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("SDK error: {0}")]
    Sdk(String),
    #[allow(dead_code)]
    #[error("Session error: {0}")]
    Session(String),
}

#[derive(Debug, Clone)]
pub enum AgentEvent {
    Text(String),
    // ToolStart is now emitted from PreToolUse hook, not from stream parsing
    SessionInit { auth_type: String },
    TokenUsage { total_tokens: u64 },
    Complete,
    Error(String),
}

/// Response from the UI for a permission request
#[derive(Debug, Clone)]
pub struct PermissionResponse {
    pub allowed: bool,
}

pub struct AgentSession {
    pub id: String,
    pub folder_path: String,
    messages: Arc<Mutex<Vec<ChatMessage>>>,
    client: Arc<Mutex<Option<ClaudeClient>>>,
    app_handle: AppHandle,
    permission_mode: Arc<RwLock<String>>,
    model: Arc<RwLock<String>>,
    extended_thinking: Arc<RwLock<bool>>,
    /// Session ID to resume from Claude CLI
    resume_session_id: Option<String>,
    /// Custom CLI path (if user has configured one)
    cli_path: Option<String>,
    /// Flag to signal abort - can be set without holding any locks
    abort_flag: Arc<AtomicBool>,
    /// Notify for abort signaling - only fires when explicitly triggered
    abort_notify: Arc<Notify>,
}

impl Clone for AgentSession {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            folder_path: self.folder_path.clone(),
            messages: self.messages.clone(),
            client: self.client.clone(),
            app_handle: self.app_handle.clone(),
            permission_mode: self.permission_mode.clone(),
            model: self.model.clone(),
            extended_thinking: self.extended_thinking.clone(),
            resume_session_id: self.resume_session_id.clone(),
            cli_path: self.cli_path.clone(),
            abort_flag: self.abort_flag.clone(),
            abort_notify: self.abort_notify.clone(),
        }
    }
}

fn string_to_permission_mode(mode: &str) -> PermissionMode {
    match mode {
        "acceptEdits" => PermissionMode::AcceptEdits,
        "bypassPermissions" => PermissionMode::BypassPermissions,
        // Note: Plan mode is not supported - it requires interactive CLI dialogs
        _ => PermissionMode::Default,
    }
}

fn string_to_model_id(model: &str) -> &'static str {
    match model {
        "opus" => "claude-opus-4-5",
        "sonnet" => "claude-sonnet-4-5",
        "haiku" => "claude-haiku-4-5",
        _ => "claude-sonnet-4-5",
    }
}

impl AgentSession {
    pub async fn new(folder_path: String, permission_mode: String, model: String, resume_session_id: Option<String>, cli_path: Option<String>, app_handle: AppHandle) -> Result<Self, AgentError> {
        let id = Uuid::new_v4().to_string();

        Ok(Self {
            id,
            folder_path,
            messages: Arc::new(Mutex::new(Vec::new())),
            client: Arc::new(Mutex::new(None)),
            app_handle,
            permission_mode: Arc::new(RwLock::new(permission_mode)),
            model: Arc::new(RwLock::new(model)),
            extended_thinking: Arc::new(RwLock::new(true)),
            resume_session_id,
            cli_path,
            abort_flag: Arc::new(AtomicBool::new(false)),
            abort_notify: Arc::new(Notify::new()),
        })
    }

    /// Get a clone of the messages for external access
    pub async fn get_messages(&self) -> Vec<ChatMessage> {
        let messages = self.messages.lock().await;
        messages.clone()
    }

    pub async fn set_permission_mode(&self, mode: String) -> Result<(), AgentError> {
        // Update stored mode
        {
            let mut current_mode = self.permission_mode.write().await;
            *current_mode = mode.clone();
        }

        // If client exists, update it via the SDK
        let client_guard = self.client.lock().await;
        if let Some(client) = client_guard.as_ref() {
            let sdk_mode = string_to_permission_mode(&mode);
            client.set_permission_mode(sdk_mode).await.map_err(|e| AgentError::Sdk(e.to_string()))?;
        }

        Ok(())
    }

    pub async fn set_model(&self, model: String) -> Result<(), AgentError> {
        // Update stored model
        {
            let mut current_model = self.model.write().await;
            *current_model = model.clone();
        }

        // If client exists, update it via the SDK control protocol
        let client_guard = self.client.lock().await;
        if let Some(client) = client_guard.as_ref() {
            let model_id = string_to_model_id(&model);
            client.set_model(Some(model_id)).await.map_err(|e| AgentError::Sdk(e.to_string()))?;
        }

        Ok(())
    }

    pub async fn set_extended_thinking(&self, enabled: bool) -> Result<(), AgentError> {
        let mut current = self.extended_thinking.write().await;
        *current = enabled;
        Ok(())
    }

    pub async fn get_permission_mode(&self) -> String {
        self.permission_mode.read().await.clone()
    }

    pub async fn get_model(&self) -> String {
        self.model.read().await.clone()
    }

    pub async fn send_message<F>(&self, message: &str, on_event: F) -> Result<(), AgentError>
    where
        F: Fn(AgentEvent) + Send + Sync + 'static,
    {
        // Clear abort flag at start of new message
        self.abort_flag.store(false, Ordering::SeqCst);

        // Store user message
        {
            let mut messages = self.messages.lock().await;
            messages.push(ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "user".to_string(),
                content: message.to_string(),
                timestamp: chrono::Utc::now().timestamp(),
            });
        }

        // Get the permission channels from Tauri state and build hooks
        let permission_channels: tauri::State<'_, PermissionChannels> = self.app_handle.state();
        let hooks = build_hooks(
            permission_channels.inner().clone(),
            self.app_handle.clone(),
            self.id.clone(),
            self.permission_mode.clone(),
            self.abort_flag.clone(),
            self.abort_notify.clone(),
        );

        // Get the current settings for the SDK options
        let current_mode = self.permission_mode.read().await.clone();
        let current_model = self.model.read().await.clone();
        let extended_thinking = *self.extended_thinking.read().await;
        let sdk_mode = string_to_permission_mode(&current_mode);
        let model_id = string_to_model_id(&current_model);

        // Build options with conditional configuration
        // Disabled tools:
        // - AskUserQuestion: CLI blocks waiting for terminal input, no programmatic answer mechanism
        // - EnterPlanMode/ExitPlanMode: Plan mode requires interactive CLI approval dialogs
        // See ASK_USER_QUESTION_ISSUE.md for research details
        let options = ClaudeAgentOptions {
            cwd: Some(PathBuf::from(&self.folder_path)),
            hooks: Some(hooks),
            disallowed_tools: vec![
                "AskUserQuestion".to_string(),
                "EnterPlanMode".to_string(),
                "ExitPlanMode".to_string(),
            ],
            permission_mode: Some(sdk_mode),
            model: Some(model_id.to_string()),
            setting_sources: Some(vec![SettingSource::User, SettingSource::Project]),
            resume: self.resume_session_id.clone(),
            max_thinking_tokens: if extended_thinking { Some(31999) } else { None },
            cli_path: self.cli_path.as_ref().map(PathBuf::from),
            ..Default::default()
        };

        // Create client if needed
        let mut client_guard = self.client.lock().await;

        if client_guard.is_none() {
            *client_guard = Some(ClaudeClient::new(options));
        }

        let client = client_guard.as_mut().unwrap();

        // Connect if not connected
        client.connect().await.map_err(|e| AgentError::Sdk(e.to_string()))?;

        // Always update the model before sending the query to ensure model changes take effect
        // This handles the case where set_model() was called when client wasn't connected
        client.set_model(Some(model_id)).await.map_err(|e| AgentError::Sdk(e.to_string()))?;

        // Send query
        client.query(message).await.map_err(|e| AgentError::Sdk(e.to_string()))?;

        let mut assistant_content = String::new();
        let mut was_aborted = false;

        // Receive responses
        let mut stream = client.receive_response();
        let mut interrupt_sent = false;

        loop {
            // Check abort flag at loop start (handles case where abort was set before loop)
            if !interrupt_sent && self.abort_flag.load(Ordering::SeqCst) {
                let _ = client.interrupt().await;
                interrupt_sent = true;
                was_aborted = true;
            }

            // Use a timeout when draining after interrupt to avoid hanging
            let stream_timeout = if interrupt_sent {
                tokio::time::Duration::from_secs(5)
            } else {
                tokio::time::Duration::from_secs(300) // 5 min for normal operation
            };

            // Use select to race between stream and abort notification
            // This ensures abort takes effect immediately, not after the next stream message
            let result = tokio::select! {
                biased;

                // Wake up immediately when abort is signaled
                _ = self.abort_notify.notified(), if !interrupt_sent => {
                    let _ = client.interrupt().await;
                    interrupt_sent = true;
                    was_aborted = true;
                    continue; // Re-enter loop to process remaining stream with short timeout
                }

                // Normal stream processing with timeout
                result = tokio::time::timeout(stream_timeout, stream.next()) => result,
            };

            match result {
                Ok(Some(msg_result)) => {
                    match msg_result {
                        Ok(msg) => {
                            match msg {
                                Message::Assistant(assistant_msg) => {
                                    // Skip processing assistant messages if we're aborting
                                    if interrupt_sent {
                                        continue;
                                    }

                                    // Extract token usage from assistant message (per API call, not cumulative)
                                    if let Some(usage) = &assistant_msg.message.usage {
                                        let input = usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                                        let cache_read = usage.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                                        on_event(AgentEvent::TokenUsage { total_tokens: input + cache_read });
                                    }

                                    for block in assistant_msg.message.content.iter() {
                                        match block {
                                            ContentBlock::Text(text_block) => {
                                                assistant_content.push_str(&text_block.text);
                                                on_event(AgentEvent::Text(text_block.text.clone()));
                                            }
                                            ContentBlock::ToolUse(_tool) => {
                                                // ToolStart is now emitted from PreToolUse hook
                                                // with full context including permission status
                                            }
                                            ContentBlock::Thinking(thinking_block) => {
                                                let thinking_id = format!("thinking-{}", Uuid::new_v4());

                                                // Emit ThinkingStart
                                                let _ = self.app_handle.emit("claude:event", &ChatEvent::ThinkingStart {
                                                    thinking_id: thinking_id.clone(),
                                                    content: thinking_block.thinking.clone(),
                                                });

                                                // Emit ThinkingEnd immediately (thinking comes as complete block)
                                                let _ = self.app_handle.emit("claude:event", &ChatEvent::ThinkingEnd {
                                                    thinking_id,
                                                });
                                            }
                                            _ => {
                                                // ToolResult should come via Message::User, not here
                                                eprintln!("[agent] Unexpected content block in Assistant message: {:?}", block);
                                            }
                                        }
                                    }
                                }
                                Message::Result(result) => {
                                    // Note: Token usage is now extracted from Assistant messages
                                    // (per API call) rather than Result (which is cumulative)
                                    let _ = result; // silence unused warning
                                    // Turn properly concluded
                                    if !was_aborted {
                                        on_event(AgentEvent::Complete);
                                    }
                                    break;
                                }
                                Message::System(sys) => {
                                    // Extract auth type from init message
                                    if sys.subtype == "init" {
                                        let api_key_source = sys.data
                                            .get("apiKeySource")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("unknown");

                                        let auth_type = match api_key_source {
                                            "none" => "Claude AI Subscription",
                                            "environment" | "settings" => "Anthropic API Key",
                                            _ => "Unknown",
                                        };

                                        on_event(AgentEvent::SessionInit {
                                            auth_type: auth_type.to_string(),
                                        });
                                    }
                                }
                                Message::User(_user_msg) => {
                                    // Tool results are now handled by the PostToolUse hook
                                    // which fires immediately when a tool completes, ensuring
                                    // proper ordering (ToolEnd before subsequent Text events)
                                }
                                _ => {}
                            }
                        }
                        Err(e) => {
                            if !was_aborted {
                                on_event(AgentEvent::Error(e.to_string()));
                            }
                            break;
                        }
                    }
                }
                Ok(None) => {
                    // Stream ended
                    break;
                }
                Err(_timeout) => {
                    if interrupt_sent {
                        // Timeout while draining after interrupt - this is expected, treat as abort
                        eprintln!("[agent] Timeout waiting for stream to drain after interrupt");
                        was_aborted = true;
                    } else {
                        // Unexpected timeout during normal operation - emit error
                        eprintln!("[agent] Stream timeout during normal operation");
                        on_event(AgentEvent::Error("Stream timeout - no response from Claude".to_string()));
                    }
                    break;
                }
            }
        }

        // Drop the stream first (it borrows from client)
        drop(stream);

        // Emit appropriate completion event
        if was_aborted {
            // Emit AbortComplete after stream is fully drained
            // This signals the frontend that the abort is complete and it can finalize
            let _ = self.app_handle.emit("claude:event", &ChatEvent::AbortComplete {
                session_id: self.id.clone(),
            });
        }
        // Note: Normal completion emits Complete via on_event in the Result branch

        // Client should still be usable with context preserved after a clean abort
        // The stream was properly drained before we get here

        // Store assistant message
        if !assistant_content.is_empty() {
            let mut messages = self.messages.lock().await;
            messages.push(ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "assistant".to_string(),
                content: assistant_content,
                timestamp: chrono::Utc::now().timestamp(),
            });
        }

        Ok(())
    }

    pub async fn abort(&self) -> Result<(), AgentError> {
        // Set abort flag - this is lock-free and immediate
        self.abort_flag.store(true, Ordering::SeqCst);

        // Signal any waiting permission prompts via Notify
        self.abort_notify.notify_waiters();

        // Note: The actual client.interrupt() is called inside the streaming loop
        // when the abort flag is checked. The AbortComplete event is emitted after
        // the stream is drained in send_message(), not here.

        Ok(())
    }

    /// Cleanup the session - abort any running operations and disconnect the client.
    /// Called when the app is closing to prevent orphaned processes.
    pub async fn cleanup(&self) {
        // First, abort any running operations
        let _ = self.abort().await;

        // Then disconnect the client
        let mut client_guard = self.client.lock().await;
        if let Some(mut client) = client_guard.take() {
            if let Err(e) = client.disconnect().await {
                eprintln!("[agent] Error disconnecting client during cleanup: {}", e);
            }
        }
    }
}
