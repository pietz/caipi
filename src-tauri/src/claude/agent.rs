use crate::commands::chat::Message as ChatMessage;
use claude_agent_sdk_rs::{
    ClaudeClient, ClaudeAgentOptions, Message, ContentBlock, ToolUseBlock, PermissionMode,
};
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
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
    PermissionRequest { id: String, tool: String, description: String },
    Complete,
    Error(String),
}

pub struct AgentSession {
    pub id: String,
    pub folder_path: String,
    pub messages: Vec<ChatMessage>,
    client: Option<ClaudeClient>,
    #[allow(dead_code)]
    pending_permissions: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
}

impl AgentSession {
    pub async fn new(folder_path: String) -> Result<Self, AgentError> {
        let id = Uuid::new_v4().to_string();

        Ok(Self {
            id,
            folder_path,
            messages: Vec::new(),
            client: None,
            pending_permissions: Arc::new(Mutex::new(HashMap::new())),
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

        // Configure the agent
        let options = ClaudeAgentOptions::builder()
            .cwd(&self.folder_path)
            .permission_mode(PermissionMode::Default)
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
                            Message::System(sys) => {
                                // Handle system messages if needed
                                if sys.subtype == "permission_request" {
                                    // Parse permission request from data
                                    if let Some(tool) = sys.data.get("tool").and_then(|v| v.as_str()) {
                                        let description = sys.data.get("description")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("Unknown operation")
                                            .to_string();

                                        on_event(AgentEvent::PermissionRequest {
                                            id: sys.uuid.clone().unwrap_or_default(),
                                            tool: tool.to_string(),
                                            description,
                                        });
                                    }
                                }
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

    pub async fn respond_permission(&mut self, _request_id: &str, allowed: bool) -> Result<(), AgentError> {
        // For now, permission responses are handled through the CLI's default permission system
        // The SDK handles permissions through PermissionMode
        if let Some(client) = &mut self.client {
            if allowed {
                // Continue execution - the SDK will handle this
            } else {
                // Interrupt current operation
                client.interrupt().await.map_err(|e| AgentError::Sdk(e.to_string()))?;
            }
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
