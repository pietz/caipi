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

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("short", 10), "short");
        assert_eq!(truncate_str("this is a long string", 10), "this is...");
    }
}
