use crate::commands::chat::Message as ChatMessage;
use claude_agent_sdk_rs::{
    ClaudeClient, ClaudeAgentOptions, Message, ContentBlock, ToolUseBlock,
    HookEvent, HookMatcher, HookCallback, HookInput, HookContext, HookJsonOutput,
    SyncHookJsonOutput, HookSpecificOutput, PreToolUseHookSpecificOutput,
    PostToolUseHookInput, PermissionMode,
};
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use thiserror::Error;
use tokio::sync::{Mutex, oneshot, RwLock};
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

/// Response from the UI for a plan approval request
#[derive(Debug, Clone)]
pub struct PlanResponse {
    pub approved: bool,
    pub comment: Option<String>,  // Feedback for the agent if rejected/requesting changes
}

/// Global permission channels - separate from session store to avoid deadlock
pub type PermissionChannels = Arc<Mutex<HashMap<String, oneshot::Sender<PermissionResponse>>>>;

/// Global plan channels - for plan approval requests
pub type PlanChannels = Arc<Mutex<HashMap<String, oneshot::Sender<PlanResponse>>>>;

pub struct AgentSession {
    pub id: String,
    pub folder_path: String,
    pub messages: Vec<ChatMessage>,
    client: Option<ClaudeClient>,
    app_handle: AppHandle,
    permission_mode: Arc<RwLock<String>>,
    model: Arc<RwLock<String>>,
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
        "opus" => "claude-opus-4-5-20251101",
        "sonnet" => "claude-sonnet-4-5-20250514",
        "haiku" => "claude-haiku-3-5-20241022",
        _ => "claude-opus-4-5-20251101",
    }
}

impl AgentSession {
    pub async fn new(folder_path: String, permission_mode: String, model: String, app_handle: AppHandle) -> Result<Self, AgentError> {
        let id = Uuid::new_v4().to_string();

        Ok(Self {
            id,
            folder_path,
            messages: Vec::new(),
            client: None,
            app_handle,
            permission_mode: Arc::new(RwLock::new(permission_mode)),
            model: Arc::new(RwLock::new(model)),
        })
    }

    pub async fn set_permission_mode(&mut self, mode: String) -> Result<(), AgentError> {
        // Update stored mode
        {
            let mut current_mode = self.permission_mode.write().await;
            *current_mode = mode.clone();
        }

        // If client exists, update it via the SDK
        if let Some(client) = &self.client {
            let sdk_mode = string_to_permission_mode(&mode);
            client.set_permission_mode(sdk_mode).await.map_err(|e| AgentError::Sdk(e.to_string()))?;
        }

        Ok(())
    }

    pub async fn set_model(&mut self, model: String) -> Result<(), AgentError> {
        // Update stored model
        {
            let mut current_model = self.model.write().await;
            *current_model = model.clone();
        }

        // Note: Model changes take effect on the next message, as the SDK doesn't support
        // changing the model mid-session. The new model will be used when building options.
        Ok(())
    }

    pub async fn send_message<F>(&mut self, message: &str, on_event: F) -> Result<(), AgentError>
    where
        F: Fn(AgentEvent) + Send + Sync + 'static,
    {
        // Store user message
        self.messages.push(ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: "user".to_string(),
            content: message.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        });

        // Get the permission channels from Tauri state
        let permission_channels: tauri::State<'_, PermissionChannels> = self.app_handle.state();
        let permission_channels = permission_channels.inner().clone();
        // Get the plan channels from Tauri state
        let plan_channels: tauri::State<'_, PlanChannels> = self.app_handle.state();
        let plan_channels = plan_channels.inner().clone();
        let app_handle = self.app_handle.clone();
        let session_id = self.id.clone();
        let permission_mode_arc = self.permission_mode.clone();
        let folder_path = self.folder_path.clone();

        let pre_tool_use_hook: HookCallback = Arc::new(move |input: HookInput, tool_use_id: Option<String>, _ctx: HookContext| {
            let permission_channels = permission_channels.clone();
            let plan_channels = plan_channels.clone();
            let app_handle = app_handle.clone();
            let session_id = session_id.clone();
            let permission_mode_arc = permission_mode_arc.clone();
            let folder_path = folder_path.clone();
            let tool_use_id = tool_use_id.clone();

            Box::pin(async move {
                // Only handle PreToolUse events
                let (tool_name, tool_input) = match &input {
                    HookInput::PreToolUse(pre_tool) => {
                        (pre_tool.tool_name.clone(), pre_tool.tool_input.clone())
                    }
                    _ => {
                        // Not a PreToolUse event, continue without intervention
                        return HookJsonOutput::Sync(SyncHookJsonOutput::default());
                    }
                };

                // Handle ExitPlanMode - show plan approval UI
                if tool_name == "ExitPlanMode" {
                    // Try to read the plan file from ~/.claude/plans/ directory
                    let plan_content = read_latest_plan_file(&folder_path).await;

                    // Create a oneshot channel for plan approval
                    let (tx, rx) = oneshot::channel();
                    let request_id = Uuid::new_v4().to_string();

                    // Store the sender in the plan channels
                    {
                        let mut channels = plan_channels.lock().await;
                        channels.insert(request_id.clone(), tx);
                    }

                    // Emit the plan ready event to the frontend
                    let _ = app_handle.emit("claude:event", serde_json::json!({
                        "type": "PlanReady",
                        "id": request_id,
                        "sessionId": session_id,
                        "toolUseId": tool_use_id,
                        "planContent": plan_content.unwrap_or_else(|| "Plan content not available".to_string()),
                    }));

                    // Wait for the response from the UI
                    match rx.await {
                        Ok(response) => {
                            if response.approved {
                                // Plan approved - allow ExitPlanMode to proceed
                                return HookJsonOutput::Sync(SyncHookJsonOutput {
                                    hook_specific_output: Some(HookSpecificOutput::PreToolUse(
                                        PreToolUseHookSpecificOutput {
                                            permission_decision: Some("allow".to_string()),
                                            permission_decision_reason: Some("User approved the plan".to_string()),
                                            updated_input: None,
                                        }
                                    )),
                                    ..Default::default()
                                });
                            } else {
                                // Plan rejected or changes requested
                                let reason = response.comment.unwrap_or_else(|| "User rejected the plan".to_string());
                                return HookJsonOutput::Sync(SyncHookJsonOutput {
                                    hook_specific_output: Some(HookSpecificOutput::PreToolUse(
                                        PreToolUseHookSpecificOutput {
                                            permission_decision: Some("deny".to_string()),
                                            permission_decision_reason: Some(reason),
                                            updated_input: None,
                                        }
                                    )),
                                    ..Default::default()
                                });
                            }
                        }
                        Err(_) => {
                            // Channel was closed
                            return HookJsonOutput::Sync(SyncHookJsonOutput {
                                hook_specific_output: Some(HookSpecificOutput::PreToolUse(
                                    PreToolUseHookSpecificOutput {
                                        permission_decision: Some("deny".to_string()),
                                        permission_decision_reason: Some("Plan approval request cancelled".to_string()),
                                        updated_input: None,
                                    }
                                )),
                                ..Default::default()
                            });
                        }
                    }
                }

                // Get the current permission mode
                let current_mode = permission_mode_arc.read().await.clone();

                // Check permission based on mode
                match current_mode.as_str() {
                    "bypassPermissions" => {
                        // Allow everything without prompting
                        return HookJsonOutput::Sync(SyncHookJsonOutput {
                            hook_specific_output: Some(HookSpecificOutput::PreToolUse(
                                PreToolUseHookSpecificOutput {
                                    permission_decision: Some("allow".to_string()),
                                    permission_decision_reason: Some("Bypass mode - all tools allowed".to_string()),
                                    updated_input: None,
                                }
                            )),
                            ..Default::default()
                        });
                    }
                    "acceptEdits" => {
                        // Auto-allow Write/Edit, only prompt for Bash
                        let is_bash = tool_name == "Bash";
                        if !is_bash {
                            return HookJsonOutput::Sync(SyncHookJsonOutput {
                                hook_specific_output: Some(HookSpecificOutput::PreToolUse(
                                    PreToolUseHookSpecificOutput {
                                        permission_decision: Some("allow".to_string()),
                                        permission_decision_reason: Some("AcceptEdits mode - file operations allowed".to_string()),
                                        updated_input: None,
                                    }
                                )),
                                ..Default::default()
                            });
                        }
                        // Fall through to prompt for Bash
                    }
                    "plan" => {
                        // In plan mode, deny all write operations (the SDK should handle this, but just in case)
                        let requires_permission = matches!(
                            tool_name.as_str(),
                            "Write" | "Edit" | "Bash" | "NotebookEdit"
                        );
                        if requires_permission {
                            return HookJsonOutput::Sync(SyncHookJsonOutput {
                                hook_specific_output: Some(HookSpecificOutput::PreToolUse(
                                    PreToolUseHookSpecificOutput {
                                        permission_decision: Some("deny".to_string()),
                                        permission_decision_reason: Some("Plan mode - write operations not allowed".to_string()),
                                        updated_input: None,
                                    }
                                )),
                                ..Default::default()
                            });
                        }
                        return HookJsonOutput::Sync(SyncHookJsonOutput {
                            hook_specific_output: Some(HookSpecificOutput::PreToolUse(
                                PreToolUseHookSpecificOutput {
                                    permission_decision: Some("allow".to_string()),
                                    permission_decision_reason: Some("Read-only operation".to_string()),
                                    updated_input: None,
                                }
                            )),
                            ..Default::default()
                        });
                    }
                    _ => {
                        // Default mode - check if this tool requires permission
                    }
                }

                // Check if this tool requires permission (for default mode and Bash in acceptEdits)
                let requires_permission = matches!(
                    tool_name.as_str(),
                    "Write" | "Edit" | "Bash" | "NotebookEdit"
                );

                if !requires_permission {
                    // Allow read-only tools without asking
                    return HookJsonOutput::Sync(SyncHookJsonOutput {
                        hook_specific_output: Some(HookSpecificOutput::PreToolUse(
                            PreToolUseHookSpecificOutput {
                                permission_decision: Some("allow".to_string()),
                                permission_decision_reason: Some("Read-only operation".to_string()),
                                updated_input: None,
                            }
                        )),
                        ..Default::default()
                    });
                }

                // Extract a description from the tool input
                let description = match tool_name.as_str() {
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
                };

                // Create a oneshot channel for this permission request
                let (tx, rx) = oneshot::channel();

                // Generate a unique request ID
                let request_id = Uuid::new_v4().to_string();

                // Store the sender in the global permission channels (keyed by request_id)
                {
                    let mut channels = permission_channels.lock().await;
                    channels.insert(request_id.clone(), tx);
                }

                // Emit the permission request event to the frontend
                let _ = app_handle.emit("claude:event", serde_json::json!({
                    "type": "PermissionRequest",
                    "id": request_id,
                    "sessionId": session_id,
                    "tool": tool_name,
                    "toolUseId": tool_use_id,
                    "description": description
                }));

                // Wait for the response from the UI
                match rx.await {
                    Ok(response) => {
                        if response.allowed {
                            HookJsonOutput::Sync(SyncHookJsonOutput {
                                hook_specific_output: Some(HookSpecificOutput::PreToolUse(
                                    PreToolUseHookSpecificOutput {
                                        permission_decision: Some("allow".to_string()),
                                        permission_decision_reason: Some("User approved".to_string()),
                                        updated_input: None,
                                    }
                                )),
                                ..Default::default()
                            })
                        } else {
                            HookJsonOutput::Sync(SyncHookJsonOutput {
                                hook_specific_output: Some(HookSpecificOutput::PreToolUse(
                                    PreToolUseHookSpecificOutput {
                                        permission_decision: Some("deny".to_string()),
                                        permission_decision_reason: Some("User denied".to_string()),
                                        updated_input: None,
                                    }
                                )),
                                ..Default::default()
                            })
                        }
                    }
                    Err(_) => {
                        // Channel was closed (e.g., session aborted)
                        HookJsonOutput::Sync(SyncHookJsonOutput {
                            hook_specific_output: Some(HookSpecificOutput::PreToolUse(
                                PreToolUseHookSpecificOutput {
                                    permission_decision: Some("deny".to_string()),
                                    permission_decision_reason: Some("Permission request cancelled".to_string()),
                                    updated_input: None,
                                }
                            )),
                            ..Default::default()
                        })
                    }
                }
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
                        let _ = app_handle.emit("claude:event", serde_json::json!({
                            "type": "ToolEnd",
                            "id": id,
                            "status": status
                        }));
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

        // Create client if needed
        let client = match &mut self.client {
            Some(c) => c,
            None => {
                let new_client = ClaudeClient::new(options);
                self.client = Some(new_client);
                self.client.as_mut().unwrap()
            }
        };

        // Connect if not connected
        client.connect().await.map_err(|e| AgentError::Sdk(e.to_string()))?;

        // Send query
        client.query(message).await.map_err(|e| AgentError::Sdk(e.to_string()))?;

        let mut assistant_content = String::new();

        // Receive responses
        {
            let mut stream = client.receive_response();
            while let Some(result) = stream.next().await {
                match result {
                    Ok(msg) => {
                        match msg {
                            Message::Assistant(assistant_msg) => {
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
                                on_event(AgentEvent::Complete);
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
                        on_event(AgentEvent::Error(e.to_string()));
                        break;
                    }
                }
            }
        }

        // Store assistant message
        if !assistant_content.is_empty() {
            self.messages.push(ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "assistant".to_string(),
                content: assistant_content,
                timestamp: chrono::Utc::now().timestamp(),
            });
        }

        Ok(())
    }

    pub async fn abort(&mut self) -> Result<(), AgentError> {
        if let Some(client) = &mut self.client {
            client.interrupt().await.map_err(|e| AgentError::Sdk(e.to_string()))?;
        }
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

/// Reads the most recent plan file from ~/.claude/plans/ directory
async fn read_latest_plan_file(folder_path: &str) -> Option<String> {
    // Plan files are stored in ~/.claude/plans/ based on the project path
    let home = std::env::var("HOME").ok()?;
    let plans_dir = std::path::Path::new(&home).join(".claude").join("plans");

    if !plans_dir.exists() {
        return None;
    }

    // Find all .md files and get the most recent one
    let mut latest_file: Option<(std::time::SystemTime, std::path::PathBuf)> = None;

    if let Ok(entries) = std::fs::read_dir(&plans_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "md").unwrap_or(false) {
                if let Ok(metadata) = path.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if latest_file.as_ref().map(|(t, _)| modified > *t).unwrap_or(true) {
                            latest_file = Some((modified, path));
                        }
                    }
                }
            }
        }
    }

    // Read the content of the latest plan file
    if let Some((_, path)) = latest_file {
        return std::fs::read_to_string(path).ok();
    }

    // If no file found in plans dir, try to find plan in project's .claude directory
    let project_claude_dir = std::path::Path::new(folder_path).join(".claude");
    if project_claude_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&project_claude_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                // Look for files that might be plan files (typically .md)
                if path.extension().map(|e| e == "md").unwrap_or(false) {
                    if let Some(name) = path.file_name() {
                        if name.to_string_lossy().contains("plan") {
                            return std::fs::read_to_string(path).ok();
                        }
                    }
                }
            }
        }
    }

    None
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
        "ExitPlanMode" => "plan ready for approval".to_string(),
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
