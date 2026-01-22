use crate::commands::chat::Message as ChatMessage;
use claude_agent_sdk_rs::{
    ClaudeClient, ClaudeAgentOptions, Message, ContentBlock, ToolUseBlock,
    HookEvent, HookMatcher, HookCallback, HookInput, HookContext, HookJsonOutput,
    SyncHookJsonOutput, HookSpecificOutput, PreToolUseHookSpecificOutput,
};
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use thiserror::Error;
use tokio::sync::{Mutex, oneshot};
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
    ToolEnd { id: String, status: String },
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
    pub messages: Vec<ChatMessage>,
    client: Option<ClaudeClient>,
    app_handle: AppHandle,
}

impl AgentSession {
    pub async fn new(folder_path: String, app_handle: AppHandle) -> Result<Self, AgentError> {
        let id = Uuid::new_v4().to_string();

        Ok(Self {
            id,
            folder_path,
            messages: Vec::new(),
            client: None,
            app_handle,
        })
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
        let app_handle = self.app_handle.clone();
        let session_id = self.id.clone();

        let pre_tool_use_hook: HookCallback = Arc::new(move |input: HookInput, _matcher: Option<String>, _ctx: HookContext| {
            let permission_channels = permission_channels.clone();
            let app_handle = app_handle.clone();
            let session_id = session_id.clone();

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

                // Check if this tool requires permission
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

        // Create hooks map
        let mut hooks: HashMap<HookEvent, Vec<HookMatcher>> = HashMap::new();
        hooks.insert(
            HookEvent::PreToolUse,
            vec![HookMatcher::builder()
                .hooks(vec![pre_tool_use_hook])
                .build()]
        );

        // Configure the agent with hooks
        let options = ClaudeAgentOptions::builder()
            .cwd(&self.folder_path)
            .hooks(hooks)
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
                                for block in &assistant_msg.message.content {
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
                                        ContentBlock::ToolResult(result) => {
                                            let status = if result.is_error.unwrap_or(false) {
                                                "error"
                                            } else {
                                                "completed"
                                            };
                                            on_event(AgentEvent::ToolEnd {
                                                id: result.tool_use_id.clone(),
                                                status: status.to_string(),
                                            });
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            Message::Result(_) => {
                                on_event(AgentEvent::Complete);
                                break;
                            }
                            Message::System(_sys) => {
                                // System messages are handled via hooks
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
                .map(|s| {
                    if s.len() > 50 {
                        format!("{}...", &s[..47])
                    } else {
                        s.to_string()
                    }
                })
                .unwrap_or("command".to_string())
        }
        _ => "...".to_string(),
    }
}
