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
            CodexItem::Reasoning { id, text } => {
                // Emit both start and end for reasoning since content arrives in completed events
                vec![
                    ChatEvent::ThinkingStart {
                        thinking_id: id.clone(),
                        content: text.clone(),
                    },
                    ChatEvent::ThinkingEnd {
                        thinking_id: id.clone(),
                    },
                ]
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

#[cfg(test)]
mod tests {
    use super::*;

    // ===================
    // Deserialization tests
    // ===================

    #[test]
    fn test_parse_thread_started_event() {
        let json = r#"{"type":"thread.started","thread_id":"thread_abc123"}"#;
        let event: CodexEvent = serde_json::from_str(json).unwrap();
        match event {
            CodexEvent::ThreadStarted { thread_id } => {
                assert_eq!(thread_id, "thread_abc123");
            }
            _ => panic!("Expected ThreadStarted event"),
        }
    }

    #[test]
    fn test_parse_turn_started_event() {
        let json = r#"{"type":"turn.started"}"#;
        let event: CodexEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(event, CodexEvent::TurnStarted));
    }

    #[test]
    fn test_parse_turn_completed_with_usage() {
        let json = r#"{"type":"turn.completed","usage":{"input_tokens":100,"cached_input_tokens":20,"output_tokens":50}}"#;
        let event: CodexEvent = serde_json::from_str(json).unwrap();
        match event {
            CodexEvent::TurnCompleted { usage: Some(u) } => {
                assert_eq!(u.input_tokens, 100);
                assert_eq!(u.cached_input_tokens, 20);
                assert_eq!(u.output_tokens, 50);
            }
            _ => panic!("Expected TurnCompleted with usage"),
        }
    }

    #[test]
    fn test_parse_turn_completed_without_usage() {
        let json = r#"{"type":"turn.completed"}"#;
        let event: CodexEvent = serde_json::from_str(json).unwrap();
        match event {
            CodexEvent::TurnCompleted { usage: None } => {}
            _ => panic!("Expected TurnCompleted without usage"),
        }
    }

    #[test]
    fn test_parse_agent_message_item() {
        let json = r#"{"type":"item.completed","item":{"type":"agent_message","id":"msg_1","text":"Hello world"}}"#;
        let event: CodexEvent = serde_json::from_str(json).unwrap();
        match event {
            CodexEvent::ItemCompleted {
                item: CodexItem::AgentMessage { id, text },
            } => {
                assert_eq!(id, "msg_1");
                assert_eq!(text, "Hello world");
            }
            _ => panic!("Expected ItemCompleted with AgentMessage"),
        }
    }

    #[test]
    fn test_parse_command_execution_item() {
        let json = r#"{"type":"item.completed","item":{"type":"command_execution","id":"cmd_1","command":"ls -la","aggregated_output":"file1\nfile2","exit_code":0,"status":"completed"}}"#;
        let event: CodexEvent = serde_json::from_str(json).unwrap();
        match event {
            CodexEvent::ItemCompleted {
                item:
                    CodexItem::CommandExecution {
                        id,
                        command,
                        aggregated_output,
                        exit_code,
                        status,
                    },
            } => {
                assert_eq!(id, "cmd_1");
                assert_eq!(command, "ls -la");
                assert_eq!(aggregated_output, "file1\nfile2");
                assert_eq!(exit_code, Some(0));
                assert_eq!(status, "completed");
            }
            _ => panic!("Expected ItemCompleted with CommandExecution"),
        }
    }

    #[test]
    fn test_parse_file_write_item() {
        let json = r#"{"type":"item.completed","item":{"type":"file_write","id":"fw_1","path":"/tmp/test.txt","content":"hello"}}"#;
        let event: CodexEvent = serde_json::from_str(json).unwrap();
        match event {
            CodexEvent::ItemCompleted {
                item: CodexItem::FileWrite { id, path, content },
            } => {
                assert_eq!(id, "fw_1");
                assert_eq!(path, "/tmp/test.txt");
                assert_eq!(content, "hello");
            }
            _ => panic!("Expected ItemCompleted with FileWrite"),
        }
    }

    #[test]
    fn test_parse_file_read_item() {
        let json = r#"{"type":"item.completed","item":{"type":"file_read","id":"fr_1","path":"/tmp/read.txt"}}"#;
        let event: CodexEvent = serde_json::from_str(json).unwrap();
        match event {
            CodexEvent::ItemCompleted {
                item: CodexItem::FileRead { id, path },
            } => {
                assert_eq!(id, "fr_1");
                assert_eq!(path, "/tmp/read.txt");
            }
            _ => panic!("Expected ItemCompleted with FileRead"),
        }
    }

    #[test]
    fn test_parse_error_item() {
        let json = r#"{"type":"item.completed","item":{"type":"error","id":"err_1","message":"Something went wrong"}}"#;
        let event: CodexEvent = serde_json::from_str(json).unwrap();
        match event {
            CodexEvent::ItemCompleted {
                item: CodexItem::Error { id, message },
            } => {
                assert_eq!(id, "err_1");
                assert_eq!(message, "Something went wrong");
            }
            _ => panic!("Expected ItemCompleted with Error"),
        }
    }

    #[test]
    fn test_parse_reasoning_item() {
        let json = r#"{"type":"item.started","item":{"type":"reasoning","id":"reason_1","text":"Thinking..."}}"#;
        let event: CodexEvent = serde_json::from_str(json).unwrap();
        match event {
            CodexEvent::ItemStarted {
                item: CodexItem::Reasoning { id, text },
            } => {
                assert_eq!(id, "reason_1");
                assert_eq!(text, "Thinking...");
            }
            _ => panic!("Expected ItemStarted with Reasoning"),
        }
    }

    #[test]
    fn test_parse_unknown_item_type() {
        let json =
            r#"{"type":"item.completed","item":{"type":"future_feature","id":"x","data":"y"}}"#;
        let event: CodexEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(
            event,
            CodexEvent::ItemCompleted {
                item: CodexItem::Unknown
            }
        ));
    }

    #[test]
    fn test_parse_invalid_json_returns_error() {
        let json = r#"{"type":"not valid json"#;
        let result: Result<CodexEvent, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_unknown_event_type_returns_error() {
        let json = r#"{"type":"unknown.event"}"#;
        let result: Result<CodexEvent, _> = serde_json::from_str(json);
        // Tagged enum with unknown type should fail to parse
        assert!(result.is_err());
    }

    // ===================
    // Translation tests
    // ===================

    #[test]
    fn test_translate_thread_started() {
        let event = CodexEvent::ThreadStarted {
            thread_id: "th_123".to_string(),
        };
        let events = translate_event(&event);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatEvent::SessionInit { auth_type } => {
                assert_eq!(auth_type, "chatgpt");
            }
            _ => panic!("Expected SessionInit event"),
        }
    }

    #[test]
    fn test_translate_turn_started_returns_empty() {
        let event = CodexEvent::TurnStarted;
        let events = translate_event(&event);
        assert!(events.is_empty());
    }

    #[test]
    fn test_translate_turn_completed_with_usage() {
        let event = CodexEvent::TurnCompleted {
            usage: Some(CodexUsage {
                input_tokens: 100,
                cached_input_tokens: 10,
                output_tokens: 50,
            }),
        };
        let events = translate_event(&event);
        assert_eq!(events.len(), 2);
        match &events[0] {
            ChatEvent::TokenUsage { total_tokens } => {
                assert_eq!(*total_tokens, 150); // 100 + 50
            }
            _ => panic!("Expected TokenUsage event"),
        }
        assert!(matches!(&events[1], ChatEvent::Complete));
    }

    #[test]
    fn test_translate_turn_completed_without_usage() {
        let event = CodexEvent::TurnCompleted { usage: None };
        let events = translate_event(&event);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], ChatEvent::Complete));
    }

    #[test]
    fn test_translate_agent_message() {
        let event = CodexEvent::ItemCompleted {
            item: CodexItem::AgentMessage {
                id: "msg_1".to_string(),
                text: "Hello!".to_string(),
            },
        };
        let events = translate_event(&event);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatEvent::Text { content } => {
                assert_eq!(content, "Hello!");
            }
            _ => panic!("Expected Text event"),
        }
    }

    #[test]
    fn test_translate_command_execution_started() {
        let event = CodexEvent::ItemStarted {
            item: CodexItem::CommandExecution {
                id: "cmd_1".to_string(),
                command: "ls".to_string(),
                aggregated_output: String::new(),
                exit_code: None,
                status: "in_progress".to_string(),
            },
        };
        let events = translate_event(&event);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatEvent::ToolStart {
                tool_use_id,
                tool_type,
                target,
                status,
                ..
            } => {
                assert_eq!(tool_use_id, "cmd_1");
                assert_eq!(tool_type, "Bash");
                assert_eq!(target, "ls");
                assert_eq!(status, "running");
            }
            _ => panic!("Expected ToolStart event"),
        }
    }

    #[test]
    fn test_translate_command_execution_success() {
        let event = CodexEvent::ItemCompleted {
            item: CodexItem::CommandExecution {
                id: "cmd_1".to_string(),
                command: "ls".to_string(),
                aggregated_output: "files".to_string(),
                exit_code: Some(0),
                status: "completed".to_string(),
            },
        };
        let events = translate_event(&event);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatEvent::ToolEnd { id, status } => {
                assert_eq!(id, "cmd_1");
                assert_eq!(status, "completed");
            }
            _ => panic!("Expected ToolEnd event"),
        }
    }

    #[test]
    fn test_translate_command_execution_failure() {
        let event = CodexEvent::ItemCompleted {
            item: CodexItem::CommandExecution {
                id: "cmd_1".to_string(),
                command: "bad_cmd".to_string(),
                aggregated_output: "error".to_string(),
                exit_code: Some(1),
                status: "completed".to_string(),
            },
        };
        let events = translate_event(&event);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatEvent::ToolEnd { id, status } => {
                assert_eq!(id, "cmd_1");
                assert_eq!(status, "error");
            }
            _ => panic!("Expected ToolEnd event"),
        }
    }

    #[test]
    fn test_translate_file_write() {
        let event = CodexEvent::ItemCompleted {
            item: CodexItem::FileWrite {
                id: "fw_1".to_string(),
                path: "/tmp/test.txt".to_string(),
                content: "data".to_string(),
            },
        };
        let events = translate_event(&event);
        assert_eq!(events.len(), 2);
        // Should emit both ToolStart and ToolEnd
        match &events[0] {
            ChatEvent::ToolStart {
                tool_use_id,
                tool_type,
                target,
                ..
            } => {
                assert_eq!(tool_use_id, "fw_1");
                assert_eq!(tool_type, "Write");
                assert_eq!(target, "/tmp/test.txt");
            }
            _ => panic!("Expected ToolStart event"),
        }
        match &events[1] {
            ChatEvent::ToolEnd { id, status } => {
                assert_eq!(id, "fw_1");
                assert_eq!(status, "completed");
            }
            _ => panic!("Expected ToolEnd event"),
        }
    }

    #[test]
    fn test_translate_file_read() {
        let event = CodexEvent::ItemCompleted {
            item: CodexItem::FileRead {
                id: "fr_1".to_string(),
                path: "/tmp/read.txt".to_string(),
            },
        };
        let events = translate_event(&event);
        assert_eq!(events.len(), 2);
        match &events[0] {
            ChatEvent::ToolStart {
                tool_use_id,
                tool_type,
                target,
                ..
            } => {
                assert_eq!(tool_use_id, "fr_1");
                assert_eq!(tool_type, "Read");
                assert_eq!(target, "/tmp/read.txt");
            }
            _ => panic!("Expected ToolStart event"),
        }
        match &events[1] {
            ChatEvent::ToolEnd { id, status } => {
                assert_eq!(id, "fr_1");
                assert_eq!(status, "completed");
            }
            _ => panic!("Expected ToolEnd event"),
        }
    }

    #[test]
    fn test_translate_reasoning_start() {
        let event = CodexEvent::ItemStarted {
            item: CodexItem::Reasoning {
                id: "r_1".to_string(),
                text: "Let me think...".to_string(),
            },
        };
        let events = translate_event(&event);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatEvent::ThinkingStart {
                thinking_id,
                content,
            } => {
                assert_eq!(thinking_id, "r_1");
                assert_eq!(content, "Let me think...");
            }
            _ => panic!("Expected ThinkingStart event"),
        }
    }

    #[test]
    fn test_translate_reasoning_completed() {
        let event = CodexEvent::ItemCompleted {
            item: CodexItem::Reasoning {
                id: "r_1".to_string(),
                text: "Done thinking".to_string(),
            },
        };
        let events = translate_event(&event);
        assert_eq!(events.len(), 2);
        // Should emit both ThinkingStart (with content) and ThinkingEnd
        match &events[0] {
            ChatEvent::ThinkingStart {
                thinking_id,
                content,
            } => {
                assert_eq!(thinking_id, "r_1");
                assert_eq!(content, "Done thinking");
            }
            _ => panic!("Expected ThinkingStart event"),
        }
        match &events[1] {
            ChatEvent::ThinkingEnd { thinking_id } => {
                assert_eq!(thinking_id, "r_1");
            }
            _ => panic!("Expected ThinkingEnd event"),
        }
    }

    #[test]
    fn test_translate_error_filters_unstable_warning() {
        let event = CodexEvent::ItemCompleted {
            item: CodexItem::Error {
                id: "err_1".to_string(),
                message: "Under-development features enabled".to_string(),
            },
        };
        let events = translate_event(&event);
        assert!(events.is_empty()); // Should be filtered out
    }

    #[test]
    fn test_translate_error_emits_real_errors() {
        let event = CodexEvent::ItemCompleted {
            item: CodexItem::Error {
                id: "err_1".to_string(),
                message: "Connection failed".to_string(),
            },
        };
        let events = translate_event(&event);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatEvent::Error { message } => {
                assert_eq!(message, "Connection failed");
            }
            _ => panic!("Expected Error event"),
        }
    }

    #[test]
    fn test_translate_unknown_item() {
        let event = CodexEvent::ItemCompleted {
            item: CodexItem::Unknown,
        };
        let events = translate_event(&event);
        assert!(events.is_empty());
    }
}
