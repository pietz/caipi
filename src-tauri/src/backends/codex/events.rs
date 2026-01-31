//! Codex JSONL event types and translation to ChatEvent.

use serde::Deserialize;

use crate::commands::chat::ChatEvent;

/// Raw event from Codex CLI JSONL output.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
pub enum CodexEvent {
    #[serde(rename = "thread.started")]
    ThreadStarted { thread_id: String },

    #[serde(rename = "turn.started")]
    TurnStarted,

    #[serde(rename = "turn.completed")]
    TurnCompleted { usage: Option<CodexUsage> },

    #[serde(rename = "item.started")]
    ItemStarted { item: CodexItem },

    #[serde(rename = "item.completed")]
    ItemCompleted { item: CodexItem },
}

/// Item types within Codex events.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
pub enum CodexItem {
    #[serde(rename = "agent_message")]
    AgentMessage { id: String, text: String },

    #[serde(rename = "reasoning")]
    Reasoning { id: String, text: String },

    #[serde(rename = "command_execution")]
    CommandExecution {
        id: String,
        command: String,
        aggregated_output: String,
        exit_code: Option<i32>,
        status: String, // "in_progress" or "completed"
    },

    #[serde(rename = "file_write")]
    FileWrite {
        id: String,
        path: String,
        #[serde(default)]
        content: String,
    },

    #[serde(rename = "file_read")]
    FileRead {
        id: String,
        path: String,
    },

    #[serde(rename = "error")]
    Error { id: String, message: String },

    #[serde(other)]
    Unknown,
}

/// Token usage from Codex.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct CodexUsage {
    pub input_tokens: u64,
    #[serde(default)]
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
}

/// Converts a CodexEvent to zero or more ChatEvents.
pub fn translate_event(event: &CodexEvent) -> Vec<ChatEvent> {
    match event {
        CodexEvent::ThreadStarted { .. } => {
            vec![ChatEvent::SessionInit {
                auth_type: "chatgpt".to_string(),
            }]
        }

        CodexEvent::TurnStarted => vec![],

        CodexEvent::TurnCompleted { usage } => {
            let mut events = vec![];
            if let Some(u) = usage {
                events.push(ChatEvent::TokenUsage {
                    total_tokens: u.input_tokens + u.output_tokens,
                });
            }
            events.push(ChatEvent::Complete);
            events
        }

        CodexEvent::ItemStarted { item } => match item {
            CodexItem::CommandExecution { id, command, .. } => {
                vec![ChatEvent::ToolStart {
                    tool_use_id: id.clone(),
                    tool_type: "Bash".to_string(),
                    target: command.clone(),
                    status: "running".to_string(),
                    input: None,
                }]
            }
            CodexItem::Reasoning { id, text } => {
                vec![ChatEvent::ThinkingStart {
                    thinking_id: id.clone(),
                    content: text.clone(),
                }]
            }
            _ => vec![],
        },

        CodexEvent::ItemCompleted { item } => match item {
            CodexItem::AgentMessage { text, .. } => {
                vec![ChatEvent::Text {
                    content: text.clone(),
                }]
            }
            CodexItem::Reasoning { id, .. } => {
                vec![ChatEvent::ThinkingEnd {
                    thinking_id: id.clone(),
                }]
            }
            CodexItem::CommandExecution { id, exit_code, .. } => {
                let status = match exit_code {
                    Some(0) => "completed",
                    Some(_) => "error",
                    None => "completed",
                };
                vec![ChatEvent::ToolEnd {
                    id: id.clone(),
                    status: status.to_string(),
                }]
            }
            CodexItem::FileWrite { id, path, .. } => {
                // Emit both start and end for file writes since we only get completed events
                vec![
                    ChatEvent::ToolStart {
                        tool_use_id: id.clone(),
                        tool_type: "Write".to_string(),
                        target: path.clone(),
                        status: "running".to_string(),
                        input: None,
                    },
                    ChatEvent::ToolEnd {
                        id: id.clone(),
                        status: "completed".to_string(),
                    },
                ]
            }
            CodexItem::FileRead { id, path } => {
                // Emit both start and end for file reads since we only get completed events
                vec![
                    ChatEvent::ToolStart {
                        tool_use_id: id.clone(),
                        tool_type: "Read".to_string(),
                        target: path.clone(),
                        status: "running".to_string(),
                        input: None,
                    },
                    ChatEvent::ToolEnd {
                        id: id.clone(),
                        status: "completed".to_string(),
                    },
                ]
            }
            CodexItem::Error { message, .. } => {
                // Only emit if it's not the unstable features warning
                if !message.contains("Under-development features") {
                    vec![ChatEvent::Error {
                        message: message.clone(),
                    }]
                } else {
                    vec![]
                }
            }
            CodexItem::Unknown => vec![],
        },
    }
}
