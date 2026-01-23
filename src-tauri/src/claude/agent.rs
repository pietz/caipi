use crate::commands::chat::{Message as ChatMessage, ChatEvent};
use claude_agent_sdk_rs::{
    ClaudeClient, ClaudeAgentOptions, Message, ContentBlock, ToolUseBlock,
    HookEvent, HookMatcher, HookCallback, HookInput, HookContext, HookJsonOutput,
    SyncHookJsonOutput, HookSpecificOutput, PreToolUseHookSpecificOutput,
    PostToolUseHookInput, PermissionMode,
};
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};
use thiserror::Error;
use tokio::sync::{Mutex, oneshot, RwLock, watch};
use uuid::Uuid;

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
    ToolStart { id: String, tool_type: String, target: String },
    #[allow(dead_code)]  // Emitted directly from PostToolUse hook now
    ToolEnd { id: String, status: String },
    SessionInit { auth_type: String },
    Complete,
    Error(String),
}

/// Response from the UI for a permission request
#[derive(Debug, Clone)]
pub struct PermissionResponse {
    pub allowed: bool,
}

/// Global permission channels - separate from session store to avoid deadlock
pub type PermissionChannels = Arc<Mutex<HashMap<String, oneshot::Sender<PermissionResponse>>>>;

pub struct AgentSession {
    pub id: String,
    pub folder_path: String,
    messages: Arc<Mutex<Vec<ChatMessage>>>,
    client: Arc<Mutex<Option<ClaudeClient>>>,
    app_handle: AppHandle,
    permission_mode: Arc<RwLock<String>>,
    model: Arc<RwLock<String>>,
    /// Flag to signal abort - can be set without holding any locks
    abort_flag: Arc<AtomicBool>,
    /// Watch channel for abort signaling - allows select! to wake up immediately
    abort_sender: watch::Sender<bool>,
    abort_receiver: watch::Receiver<bool>,
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
            abort_flag: self.abort_flag.clone(),
            abort_sender: self.abort_sender.clone(),
            abort_receiver: self.abort_receiver.clone(),
        }
    }
}

fn string_to_permission_mode(mode: &str) -> PermissionMode {
    match mode {
        "acceptEdits" => PermissionMode::AcceptEdits,
        "plan" => PermissionMode::Plan,
        "bypassPermissions" => PermissionMode::BypassPermissions,
        _ => PermissionMode::Default,
    }
}

fn string_to_model_id(model: &str) -> &'static str {
    match model {
        "opus" => "claude-opus-4-5",
        "sonnet" => "claude-sonnet-4-5",
        "haiku" => "claude-haiku-4-5",
        _ => "claude-opus-4-5",
    }
}

// ============================================================================
// Permission Hook Helpers
// ============================================================================

/// Create an "allow" response for the pre-tool-use hook
fn allow_response(reason: &str) -> HookJsonOutput {
    HookJsonOutput::Sync(SyncHookJsonOutput {
        hook_specific_output: Some(HookSpecificOutput::PreToolUse(
            PreToolUseHookSpecificOutput {
                permission_decision: Some("allow".to_string()),
                permission_decision_reason: Some(reason.to_string()),
                updated_input: None,
            }
        )),
        ..Default::default()
    })
}

/// Create a "deny" response for the pre-tool-use hook
fn deny_response(reason: &str) -> HookJsonOutput {
    HookJsonOutput::Sync(SyncHookJsonOutput {
        hook_specific_output: Some(HookSpecificOutput::PreToolUse(
            PreToolUseHookSpecificOutput {
                permission_decision: Some("deny".to_string()),
                permission_decision_reason: Some(reason.to_string()),
                updated_input: None,
            }
        )),
        ..Default::default()
    })
}

/// Check if the session has been aborted
fn check_abort_decision(abort_flag: &Arc<AtomicBool>) -> Option<HookJsonOutput> {
    if abort_flag.load(Ordering::SeqCst) {
        Some(deny_response("Session aborted"))
    } else {
        None
    }
}

/// Extract tool name and input from hook input
fn extract_tool_info(input: &HookInput) -> Option<(String, serde_json::Value)> {
    match input {
        HookInput::PreToolUse(pre_tool) => {
            Some((pre_tool.tool_name.clone(), pre_tool.tool_input.clone()))
        }
        _ => None,
    }
}

/// Check if the permission mode allows the tool without prompting
fn check_mode_decision(mode: &str, tool_name: &str) -> Option<HookJsonOutput> {
    match mode {
        "bypassPermissions" => Some(allow_response("Bypass mode - all tools allowed")),
        "acceptEdits" if tool_name != "Bash" => {
            Some(allow_response("AcceptEdits mode - file operations allowed"))
        }
        _ => None,
    }
}

/// Check if the tool requires permission prompting
fn requires_permission(tool_name: &str) -> bool {
    matches!(tool_name, "Write" | "Edit" | "Bash" | "NotebookEdit")
}

/// Build a human-readable description for the permission prompt
fn build_permission_description(tool_name: &str, tool_input: &serde_json::Value) -> String {
    match tool_name {
        "Write" | "Edit" => {
            tool_input.get("file_path")
                .and_then(|v| v.as_str())
                .map(|p| format!("Modify file: {}", p))
                .unwrap_or_else(|| format!("Use tool: {}", tool_name))
        }
        "Bash" => {
            tool_input.get("command")
                .and_then(|v| v.as_str())
                .map(|cmd| {
                    if cmd.len() > 80 {
                        format!("Run command: {}...", &cmd[..77])
                    } else {
                        format!("Run command: {}", cmd)
                    }
                })
                .unwrap_or_else(|| "Run bash command".to_string())
        }
        _ => format!("Use tool: {}", tool_name),
    }
}

/// Prompt the user for permission and await their response
async fn prompt_user_permission(
    permission_channels: PermissionChannels,
    app_handle: AppHandle,
    session_id: String,
    tool_name: String,
    tool_use_id: Option<String>,
    description: String,
) -> HookJsonOutput {
    let (tx, rx) = oneshot::channel();
    let request_id = Uuid::new_v4().to_string();

    // Store sender in the global permission channels
    {
        let mut channels = permission_channels.lock().await;
        channels.insert(request_id.clone(), tx);
    }

    // Emit permission request event to frontend
    let _ = app_handle.emit("claude:event", &ChatEvent::PermissionRequest {
        id: request_id,
        session_id,
        tool: tool_name,
        tool_use_id,
        description,
    });

    // Await user response
    match rx.await {
        Ok(response) if response.allowed => allow_response("User approved"),
        Ok(_) => deny_response("User denied"),
        Err(_) => deny_response("Permission request cancelled"),
    }
}

impl AgentSession {
    pub async fn new(folder_path: String, permission_mode: String, model: String, app_handle: AppHandle) -> Result<Self, AgentError> {
        let id = Uuid::new_v4().to_string();
        let (abort_sender, abort_receiver) = watch::channel(false);

        Ok(Self {
            id,
            folder_path,
            messages: Arc::new(Mutex::new(Vec::new())),
            client: Arc::new(Mutex::new(None)),
            app_handle,
            permission_mode: Arc::new(RwLock::new(permission_mode)),
            model: Arc::new(RwLock::new(model)),
            abort_flag: Arc::new(AtomicBool::new(false)),
            abort_sender,
            abort_receiver,
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

        // Note: Model changes take effect on the next message, as the SDK doesn't support
        // changing the model mid-session. The new model will be used when building options.
        Ok(())
    }

    pub async fn send_message<F>(&self, message: &str, on_event: F) -> Result<(), AgentError>
    where
        F: Fn(AgentEvent) + Send + Sync + 'static,
    {
        // Clear abort signals at start of new message
        self.abort_flag.store(false, Ordering::SeqCst);
        let _ = self.abort_sender.send(false);

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

        // Get the permission channels from Tauri state
        let permission_channels: tauri::State<'_, PermissionChannels> = self.app_handle.state();
        let permission_channels = permission_channels.inner().clone();
        let app_handle = self.app_handle.clone();
        let session_id = self.id.clone();
        let permission_mode_arc = self.permission_mode.clone();
        let abort_flag = self.abort_flag.clone();

        let pre_tool_use_hook: HookCallback = Arc::new(move |input: HookInput, tool_use_id: Option<String>, _ctx: HookContext| {
            let permission_channels = permission_channels.clone();
            let app_handle = app_handle.clone();
            let session_id = session_id.clone();
            let permission_mode_arc = permission_mode_arc.clone();
            let abort_flag = abort_flag.clone();

            Box::pin(async move {
                // 1. Check if abort was requested
                if let Some(deny) = check_abort_decision(&abort_flag) {
                    return deny;
                }

                // 2. Extract tool info (only handle PreToolUse events)
                let (tool_name, tool_input) = match extract_tool_info(&input) {
                    Some(info) => info,
                    None => return HookJsonOutput::Sync(SyncHookJsonOutput::default()),
                };

                // 3. Check permission mode for auto-decisions
                let current_mode = permission_mode_arc.read().await.clone();
                if let Some(decision) = check_mode_decision(&current_mode, &tool_name) {
                    return decision;
                }

                // 4. Check if this tool requires permission prompting
                if !requires_permission(&tool_name) {
                    return allow_response("Read-only operation");
                }

                // 5. Build description and prompt user
                let description = build_permission_description(&tool_name, &tool_input);
                prompt_user_permission(
                    permission_channels,
                    app_handle,
                    session_id,
                    tool_name,
                    tool_use_id,
                    description,
                ).await
            })
        });

        // Create PostToolUse hook to emit ToolEnd immediately when a tool finishes
        let app_handle_post = self.app_handle.clone();
        let post_tool_use_hook: HookCallback = Arc::new(move |input: HookInput, tool_use_id: Option<String>, _ctx: HookContext| {
            let app_handle = app_handle_post.clone();
            let tool_use_id = tool_use_id.clone();

            Box::pin(async move {
                if let HookInput::PostToolUse(PostToolUseHookInput { tool_response, .. }) = &input {
                    // Determine if the tool errored by checking the response
                    let is_error = tool_response.get("is_error")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    let status = if is_error { "error" } else { "completed" };

                    // Emit ToolEnd event immediately
                    if let Some(id) = tool_use_id {
                        let _ = app_handle.emit("claude:event", &ChatEvent::ToolEnd {
                            id: id,
                            status: status.to_string(),
                        });
                    }
                }

                // Don't modify anything, just return default
                HookJsonOutput::Sync(SyncHookJsonOutput::default())
            })
        });

        // Create hooks map
        let mut hooks: HashMap<HookEvent, Vec<HookMatcher>> = HashMap::new();
        hooks.insert(
            HookEvent::PreToolUse,
            vec![HookMatcher::builder()
                .hooks(vec![pre_tool_use_hook])
                .build()]
        );
        hooks.insert(
            HookEvent::PostToolUse,
            vec![HookMatcher::builder()
                .hooks(vec![post_tool_use_hook])
                .build()]
        );

        // Get the current settings for the SDK options
        let current_mode = self.permission_mode.read().await.clone();
        let current_model = self.model.read().await.clone();
        let sdk_mode = string_to_permission_mode(&current_mode);
        let model_id = string_to_model_id(&current_model);

        let options = ClaudeAgentOptions::builder()
            .cwd(&self.folder_path)
            .hooks(hooks)
            .permission_mode(sdk_mode)
            .model(model_id)
            .build();

        // Create client if needed and perform all client operations while holding the lock
        let mut client_guard = self.client.lock().await;

        // Initialize client if needed
        if client_guard.is_none() {
            *client_guard = Some(ClaudeClient::new(options));
        }

        let client = client_guard.as_mut().unwrap();

        // Connect if not connected
        client.connect().await.map_err(|e| AgentError::Sdk(e.to_string()))?;

        // Send query
        client.query(message).await.map_err(|e| AgentError::Sdk(e.to_string()))?;

        let mut assistant_content = String::new();
        let mut abort_receiver = self.abort_receiver.clone();
        let mut was_aborted = false;

        // Receive responses - use select! to race between stream and abort signal
        let mut stream = client.receive_response();
        let mut interrupt_sent = false;

        loop {
            // If abort requested and we haven't sent interrupt yet, send it now
            // but continue draining the stream to properly conclude the turn
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

            tokio::select! {
                // Check for abort signal (only if we haven't sent interrupt yet)
                _ = abort_receiver.changed(), if !interrupt_sent => {
                    if *abort_receiver.borrow() {
                        // Abort was requested - interrupt the client
                        let _ = client.interrupt().await;
                        interrupt_sent = true;
                        was_aborted = true;
                        // Don't break - continue draining the stream
                    }
                }
                // Process next stream item with timeout
                result = tokio::time::timeout(stream_timeout, stream.next()) => {
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
                                            for block in assistant_msg.message.content.iter() {
                                                match block {
                                                    ContentBlock::Text(text_block) => {
                                                        assistant_content.push_str(&text_block.text);
                                                        on_event(AgentEvent::Text(text_block.text.clone()));
                                                    }
                                                    ContentBlock::ToolUse(tool) => {
                                                        let target = extract_tool_target(tool);
                                                        on_event(AgentEvent::ToolStart {
                                                            id: tool.id.clone(),
                                                            tool_type: tool.name.clone(),
                                                            target,
                                                        });
                                                    }
                                                    _ => {
                                                        // ToolResult should come via Message::User, not here
                                                        eprintln!("[agent] Unexpected content block in Assistant message: {:?}", block);
                                                    }
                                                }
                                            }
                                        }
                                        Message::Result(_) => {
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
                            // Timeout while draining after interrupt - force cleanup
                            eprintln!("[agent] Timeout waiting for stream to drain after interrupt");
                            break;
                        }
                    }
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

        // Signal the watch channel - this will wake up any select! waiting on it
        let _ = self.abort_sender.send(true);

        // Note: The actual client.interrupt() is now called inside the streaming loop
        // when the abort signal is received via select!, because that's where we have
        // the lock. The AbortComplete event is emitted after the stream is drained
        // in send_message(), not here.

        Ok(())
    }
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count > max_chars {
        let truncated: String = s.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{}...", truncated)
    } else {
        s.to_string()
    }
}

fn extract_tool_target(tool: &ToolUseBlock) -> String {
    // Extract the target (file path, pattern, etc.) from tool input
    match tool.name.as_str() {
        "Read" | "Write" | "Edit" => {
            tool.input.get("file_path")
                .or_else(|| tool.input.get("path"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string()
        }
        "Glob" => {
            tool.input.get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("*")
                .to_string()
        }
        "Grep" => {
            tool.input.get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("...")
                .to_string()
        }
        "Bash" => {
            tool.input.get("command")
                .and_then(|v| v.as_str())
                .map(|s| truncate_str(s, 50))
                .unwrap_or("command".to_string())
        }
        "WebSearch" => {
            tool.input.get("query")
                .and_then(|v| v.as_str())
                .map(|s| truncate_str(s, 50))
                .unwrap_or("searching...".to_string())
        }
        "WebFetch" => {
            tool.input.get("url")
                .and_then(|v| v.as_str())
                .map(|s| truncate_str(s, 50))
                .unwrap_or("fetching...".to_string())
        }
        "Skill" => {
            tool.input.get("skill")
                .and_then(|v| v.as_str())
                .unwrap_or("skill")
                .to_string()
        }
        "Task" => {
            tool.input.get("description")
                .or_else(|| tool.input.get("prompt"))
                .and_then(|v| v.as_str())
                .map(|s| truncate_str(s, 50))
                .unwrap_or("task".to_string())
        }
        "AskUserQuestion" => "asking question...".to_string(),
        "NotebookEdit" => {
            tool.input.get("notebook_path")
                .and_then(|v| v.as_str())
                .unwrap_or("notebook")
                .to_string()
        }
        _ => {
            // Try common field names for unknown tools
            let fields = ["file_path", "path", "pattern", "command", "url", "query", "skill", "prompt", "subject", "name"];
            for field in fields {
                if let Some(val) = tool.input.get(field).and_then(|v| v.as_str()) {
                    // Prefix with tool name for context
                    let detail = truncate_str(val, 40);
                    return format!("{}: {}", tool.name, detail);
                }
            }
            // Fallback: show tool name only
            tool.name.clone()
        }
    }
}
