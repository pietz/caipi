use claude_agent_sdk_rs::ToolUseBlock;

/// Truncate a string to a maximum number of characters, adding "..." if truncated
pub fn truncate_str(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count > max_chars {
        let truncated: String = s.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{}...", truncated)
    } else {
        s.to_string()
    }
}

/// Extract the target (file path, pattern, etc.) from a tool's input for display
#[allow(dead_code)]
pub fn extract_tool_target(tool: &ToolUseBlock) -> String {
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
                .unwrap_or_else(|| "command".to_string())
        }
        "WebSearch" => {
            tool.input.get("query")
                .and_then(|v| v.as_str())
                .map(|s| truncate_str(s, 50))
                .unwrap_or_else(|| "searching...".to_string())
        }
        "WebFetch" => {
            tool.input.get("url")
                .and_then(|v| v.as_str())
                .map(|s| truncate_str(s, 50))
                .unwrap_or_else(|| "fetching...".to_string())
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
                .unwrap_or_else(|| "task".to_string())
        }
        "AskUserQuestion" => "asking question...".to_string(),
        "NotebookEdit" => {
            tool.input.get("notebook_path")
                .and_then(|v| v.as_str())
                .unwrap_or("notebook")
                .to_string()
        }
        "TaskCreate" => {
            tool.input.get("subject")
                .and_then(|v| v.as_str())
                .map(|s| truncate_str(s, 50))
                .unwrap_or_else(|| "new task".to_string())
        }
        "TaskUpdate" => {
            tool.input.get("taskId")
                .and_then(|v| v.as_str())
                .map(|id| format!("task {}", truncate_str(id, 20)))
                .unwrap_or_else(|| "task".to_string())
        }
        "TaskList" | "TaskGet" => "tasks".to_string(),
        "TodoWrite" => {
            // Count how many todos in the array
            tool.input.get("todos")
                .and_then(|v| v.as_array())
                .map(|arr| format!("{} todo(s)", arr.len()))
                .unwrap_or_else(|| "todos".to_string())
        }
        "TodoRead" => "reading todos".to_string(),
        _ => {
            // Try common field names for unknown tools
            let fields = ["file_path", "path", "pattern", "command", "url", "query", "skill", "prompt", "subject", "name"];
            for field in fields {
                if let Some(val) = tool.input.get(field).and_then(|v| v.as_str()) {
                    let detail = truncate_str(val, 40);
                    return format!("{}: {}", tool.name, detail);
                }
            }
            // Fallback: show tool name only
            tool.name.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ========================================================================
    // truncate_str tests
    // ========================================================================

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("short", 10), "short");
        assert_eq!(truncate_str("this is a long string", 10), "this is...");
    }

    #[test]
    fn test_truncate_exact_boundary() {
        // String exactly at limit stays unchanged
        assert_eq!(truncate_str("exactly10c", 10), "exactly10c");
        assert_eq!(truncate_str("12345", 5), "12345");
    }

    #[test]
    fn test_truncate_one_over() {
        // String one char over limit gets truncated
        assert_eq!(truncate_str("12345678901", 10), "1234567...");
        assert_eq!(truncate_str("123456", 5), "12...");
    }

    #[test]
    fn test_truncate_unicode() {
        // Multi-byte characters (emoji, CJK) handled correctly (count chars, not bytes)
        assert_eq!(truncate_str("Hello👋World", 10), "Hello👋W...");
        // 4 chars fits in 4 chars limit (no truncation needed)
        assert_eq!(truncate_str("你好世界", 4), "你好世界");
        // 8 chars into 5: take 2 + "..." = 5
        assert_eq!(truncate_str("你好世界test", 5), "你好...");
        // Emoji: 3 chars fits in 3
        assert_eq!(truncate_str("🚀🎉🔥", 3), "🚀🎉🔥");
        // 4 emoji chars > 3, so truncate: take 0 chars + "..." (not enough space for any char + ellipsis)
        assert_eq!(truncate_str("🚀🎉🔥💯", 3), "...");
        // 4 chars > 4 should not truncate
        assert_eq!(truncate_str("🚀🎉🔥💯", 4), "🚀🎉🔥💯");
        // 4 chars into 5 fits
        assert_eq!(truncate_str("🚀🎉🔥💯", 5), "🚀🎉🔥💯");
    }

    #[test]
    fn test_truncate_empty() {
        // Empty string returns empty
        assert_eq!(truncate_str("", 10), "");
        assert_eq!(truncate_str("", 0), "");
    }

    // ========================================================================
    // extract_tool_target tests
    // ========================================================================

    #[test]
    fn test_extract_target_read() {
        let tool = ToolUseBlock {
            id: "test".to_string(),
            name: "Read".to_string(),
            input: json!({"file_path": "/path/to/file.txt"}),
        };
        assert_eq!(extract_tool_target(&tool), "/path/to/file.txt");
    }

    #[test]
    fn test_extract_target_write() {
        let tool = ToolUseBlock {
            id: "test".to_string(),
            name: "Write".to_string(),
            input: json!({"file_path": "/output/data.json"}),
        };
        assert_eq!(extract_tool_target(&tool), "/output/data.json");
    }

    #[test]
    fn test_extract_target_edit() {
        let tool = ToolUseBlock {
            id: "test".to_string(),
            name: "Edit".to_string(),
            input: json!({"file_path": "/src/main.rs"}),
        };
        assert_eq!(extract_tool_target(&tool), "/src/main.rs");
    }

    #[test]
    fn test_extract_target_glob() {
        let tool = ToolUseBlock {
            id: "test".to_string(),
            name: "Glob".to_string(),
            input: json!({"pattern": "**/*.rs"}),
        };
        assert_eq!(extract_tool_target(&tool), "**/*.rs");
    }

    #[test]
    fn test_extract_target_grep() {
        let tool = ToolUseBlock {
            id: "test".to_string(),
            name: "Grep".to_string(),
            input: json!({"pattern": "TODO"}),
        };
        assert_eq!(extract_tool_target(&tool), "TODO");
    }

    #[test]
    fn test_extract_target_bash() {
        // Bash tool extracts and truncates command
        let tool = ToolUseBlock {
            id: "test".to_string(),
            name: "Bash".to_string(),
            input: json!({"command": "ls -la"}),
        };
        assert_eq!(extract_tool_target(&tool), "ls -la");

        // Test truncation for long commands
        let long_command = "echo this is a very long command that should be truncated to fifty characters max";
        let tool_long = ToolUseBlock {
            id: "test".to_string(),
            name: "Bash".to_string(),
            input: json!({"command": long_command}),
        };
        let result = extract_tool_target(&tool_long);
        assert_eq!(result.len(), 50); // 47 chars + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_extract_target_websearch() {
        let tool = ToolUseBlock {
            id: "test".to_string(),
            name: "WebSearch".to_string(),
            input: json!({"query": "rust programming"}),
        };
        assert_eq!(extract_tool_target(&tool), "rust programming");

        // Test truncation for long queries
        let long_query = "how to write a very long search query that exceeds the fifty character limit";
        let tool_long = ToolUseBlock {
            id: "test".to_string(),
            name: "WebSearch".to_string(),
            input: json!({"query": long_query}),
        };
        let result = extract_tool_target(&tool_long);
        assert_eq!(result.len(), 50);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_extract_target_task() {
        let tool = ToolUseBlock {
            id: "test".to_string(),
            name: "Task".to_string(),
            input: json!({"description": "Analyze the codebase"}),
        };
        assert_eq!(extract_tool_target(&tool), "Analyze the codebase");

        // Test with prompt field as fallback
        let tool_prompt = ToolUseBlock {
            id: "test".to_string(),
            name: "Task".to_string(),
            input: json!({"prompt": "Search for bugs"}),
        };
        assert_eq!(extract_tool_target(&tool_prompt), "Search for bugs");
    }

    #[test]
    fn test_extract_target_todowrite() {
        // TodoWrite shows count
        let tool = ToolUseBlock {
            id: "test".to_string(),
            name: "TodoWrite".to_string(),
            input: json!({"todos": [
                {"task": "task 1"},
                {"task": "task 2"},
                {"task": "task 3"}
            ]}),
        };
        assert_eq!(extract_tool_target(&tool), "3 todo(s)");

        // Empty array
        let tool_empty = ToolUseBlock {
            id: "test".to_string(),
            name: "TodoWrite".to_string(),
            input: json!({"todos": []}),
        };
        assert_eq!(extract_tool_target(&tool_empty), "0 todo(s)");
    }

    #[test]
    fn test_extract_target_unknown() {
        // Unknown tool tries common fields then falls back to name

        // Test with file_path field
        let tool_with_path = ToolUseBlock {
            id: "test".to_string(),
            name: "UnknownTool".to_string(),
            input: json!({"file_path": "/some/file.txt"}),
        };
        assert_eq!(extract_tool_target(&tool_with_path), "UnknownTool: /some/file.txt");

        // Test with pattern field
        let tool_with_pattern = ToolUseBlock {
            id: "test".to_string(),
            name: "CustomSearch".to_string(),
            input: json!({"pattern": "search_term"}),
        };
        assert_eq!(extract_tool_target(&tool_with_pattern), "CustomSearch: search_term");

        // Test with no known fields - should fall back to tool name
        let tool_no_fields = ToolUseBlock {
            id: "test".to_string(),
            name: "MyCustomTool".to_string(),
            input: json!({"unknown_field": "value"}),
        };
        assert_eq!(extract_tool_target(&tool_no_fields), "MyCustomTool");

        // Test truncation in unknown tool with long field value
        let long_value = "this is a very long value that exceeds the forty character limit for unknown tools";
        let tool_long = ToolUseBlock {
            id: "test".to_string(),
            name: "LongTool".to_string(),
            input: json!({"command": long_value}),
        };
        let result = extract_tool_target(&tool_long);
        assert!(result.starts_with("LongTool: "));
        assert!(result.ends_with("..."));
        // "LongTool: " is 10 chars, truncated value should be 40 chars total
        assert_eq!(result.len(), "LongTool: ".len() + 40);
    }
}
