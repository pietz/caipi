use serde_json::Value;

/// Extract the target (file path, pattern, etc.) from a tool's input for display
pub fn extract_tool_target(tool_name: &str, tool_input: &Value) -> String {
    match tool_name {
        "Read" | "Write" | "Edit" => tool_input
            .get("file_path")
            .or_else(|| tool_input.get("path"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        "Glob" => tool_input
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("*")
            .to_string(),
        "Grep" => tool_input
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("...")
            .to_string(),
        "Bash" => {
            // Prefer description if available (human-readable)
            tool_input
                .get("description")
                .and_then(|v| v.as_str())
                .or_else(|| {
                    // Fall back to command if no description
                    tool_input.get("command").and_then(|v| v.as_str())
                })
                .unwrap_or("command")
                .to_string()
        }
        "WebSearch" => tool_input
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("searching...")
            .to_string(),
        "WebFetch" => tool_input
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("fetching...")
            .to_string(),
        "Skill" => tool_input
            .get("skill")
            .and_then(|v| v.as_str())
            .unwrap_or("skill")
            .to_string(),
        "Task" => tool_input
            .get("description")
            .or_else(|| tool_input.get("prompt"))
            .and_then(|v| v.as_str())
            .unwrap_or("task")
            .to_string(),
        "AskUserQuestion" => "asking question...".to_string(),
        "NotebookEdit" => tool_input
            .get("notebook_path")
            .and_then(|v| v.as_str())
            .unwrap_or("notebook")
            .to_string(),
        "TaskCreate" => tool_input
            .get("subject")
            .and_then(|v| v.as_str())
            .unwrap_or("new task")
            .to_string(),
        "TaskUpdate" => tool_input
            .get("taskId")
            .and_then(|v| v.as_str())
            .map(|id| format!("task {}", id))
            .unwrap_or_else(|| "task".to_string()),
        "TaskList" | "TaskGet" => "tasks".to_string(),
        "TodoWrite" => {
            // Count how many todos in the array
            tool_input
                .get("todos")
                .and_then(|v| v.as_array())
                .map(|arr| format!("{} todo(s)", arr.len()))
                .unwrap_or_else(|| "todos".to_string())
        }
        "TodoRead" => "reading todos".to_string(),
        _ => {
            // Try common field names for unknown tools
            let fields = [
                "file_path",
                "path",
                "pattern",
                "command",
                "url",
                "query",
                "skill",
                "prompt",
                "subject",
                "name",
            ];
            for field in fields {
                if let Some(val) = tool_input.get(field).and_then(|v| v.as_str()) {
                    return format!("{}: {}", tool_name, val);
                }
            }
            // Fallback: show tool name only
            tool_name.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_target_read() {
        let input = json!({"file_path": "/path/to/file.txt"});
        assert_eq!(extract_tool_target("Read", &input), "/path/to/file.txt");
    }

    #[test]
    fn test_extract_target_write() {
        let input = json!({"file_path": "/output/data.json"});
        assert_eq!(extract_tool_target("Write", &input), "/output/data.json");
    }

    #[test]
    fn test_extract_target_edit() {
        let input = json!({"file_path": "/src/main.rs"});
        assert_eq!(extract_tool_target("Edit", &input), "/src/main.rs");
    }

    #[test]
    fn test_extract_target_glob() {
        let input = json!({"pattern": "**/*.rs"});
        assert_eq!(extract_tool_target("Glob", &input), "**/*.rs");
    }

    #[test]
    fn test_extract_target_grep() {
        let input = json!({"pattern": "TODO"});
        assert_eq!(extract_tool_target("Grep", &input), "TODO");
    }

    #[test]
    fn test_extract_target_bash() {
        // Bash tool prefers description over command
        let input_with_desc = json!({
            "command": "git commit -m 'Fix bug'",
            "description": "Create commit with fix message"
        });
        assert_eq!(
            extract_tool_target("Bash", &input_with_desc),
            "Create commit with fix message"
        );

        // Falls back to command if no description
        let input = json!({"command": "ls -la"});
        assert_eq!(extract_tool_target("Bash", &input), "ls -la");

        // Long descriptions are passed through (CSS handles truncation)
        let long_desc = "This is a very long description that would have been truncated before";
        let input_long_desc = json!({
            "command": "some command",
            "description": long_desc
        });
        assert_eq!(extract_tool_target("Bash", &input_long_desc), long_desc);
    }

    #[test]
    fn test_extract_target_websearch() {
        let input = json!({"query": "rust programming"});
        assert_eq!(extract_tool_target("WebSearch", &input), "rust programming");

        // Long queries are passed through (CSS handles truncation)
        let long_query = "how to write a very long search query that exceeds any character limit";
        let input_long = json!({"query": long_query});
        assert_eq!(extract_tool_target("WebSearch", &input_long), long_query);
    }

    #[test]
    fn test_extract_target_task() {
        let input = json!({"description": "Analyze the codebase"});
        assert_eq!(extract_tool_target("Task", &input), "Analyze the codebase");

        // Test with prompt field as fallback
        let input_prompt = json!({"prompt": "Search for bugs"});
        assert_eq!(
            extract_tool_target("Task", &input_prompt),
            "Search for bugs"
        );
    }

    #[test]
    fn test_extract_target_todowrite() {
        // TodoWrite shows count
        let input = json!({"todos": [
            {"task": "task 1"},
            {"task": "task 2"},
            {"task": "task 3"}
        ]});
        assert_eq!(extract_tool_target("TodoWrite", &input), "3 todo(s)");

        // Empty array
        let input_empty = json!({"todos": []});
        assert_eq!(extract_tool_target("TodoWrite", &input_empty), "0 todo(s)");
    }

    #[test]
    fn test_extract_target_unknown() {
        // Unknown tool tries common fields then falls back to name

        // Test with file_path field
        let input_with_path = json!({"file_path": "/some/file.txt"});
        assert_eq!(
            extract_tool_target("UnknownTool", &input_with_path),
            "UnknownTool: /some/file.txt"
        );

        // Test with pattern field
        let input_with_pattern = json!({"pattern": "search_term"});
        assert_eq!(
            extract_tool_target("CustomSearch", &input_with_pattern),
            "CustomSearch: search_term"
        );

        // Test with no known fields - should fall back to tool name
        let input_no_fields = json!({"unknown_field": "value"});
        assert_eq!(
            extract_tool_target("MyCustomTool", &input_no_fields),
            "MyCustomTool"
        );

        // Long values are passed through (CSS handles truncation)
        let long_value = "this is a very long value that would have been truncated before";
        let input_long = json!({"command": long_value});
        let result = extract_tool_target("LongTool", &input_long);
        assert_eq!(result, format!("LongTool: {}", long_value));
    }
}
