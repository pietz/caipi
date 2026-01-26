//! Claude settings parser for ~/.claude/settings.json
//!
//! This module handles reading user settings and checking if tools are allowed
//! based on the permissions.allow list.

use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

/// Permissions configuration from settings.json
#[derive(Debug, Deserialize, Default, Clone)]
pub struct Permissions {
    #[serde(default)]
    pub allow: Vec<String>,
    /// Deny list - reserved for future implementation
    #[serde(default)]
    #[allow(dead_code)]
    pub deny: Vec<String>,
}

/// Partial settings structure - we only care about permissions
#[derive(Debug, Deserialize, Default, Clone)]
pub struct ClaudeSettings {
    #[serde(default)]
    pub permissions: Permissions,
}

/// Get the path to the user's Claude settings file
fn get_settings_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".claude").join("settings.json"))
}

/// Load user settings from ~/.claude/settings.json
pub fn load_user_settings() -> Option<ClaudeSettings> {
    let path = get_settings_path()?;
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Check if a tool invocation is allowed by the user's settings
///
/// Pattern formats supported:
/// - `"WebFetch"` - entire tool allowed
/// - `"Skill(email)"` - tool with specific argument
/// - `"Bash(ls:*)"` - bash with command prefix (`:*` means any suffix)
/// - `"Bash(uv init)"` - exact command match (no `:*`)
pub fn is_tool_allowed(settings: &ClaudeSettings, tool_name: &str, tool_input: &serde_json::Value) -> bool {
    for pattern in &settings.permissions.allow {
        if matches_pattern(pattern, tool_name, tool_input) {
            return true;
        }
    }
    false
}

/// Check if a single pattern matches the tool invocation
fn matches_pattern(pattern: &str, tool_name: &str, tool_input: &serde_json::Value) -> bool {
    // Check for pattern with arguments: "Tool(arg)" or "Tool(prefix:*)"
    if let Some(paren_pos) = pattern.find('(') {
        if !pattern.ends_with(')') {
            return false;
        }

        let pattern_tool = &pattern[..paren_pos];
        let pattern_arg = &pattern[paren_pos + 1..pattern.len() - 1];

        // Tool name must match
        if pattern_tool != tool_name {
            return false;
        }

        // Check the argument based on tool type
        match tool_name {
            "Bash" => {
                let command = tool_input
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if pattern_arg.ends_with(":*") {
                    // Prefix match: "ls:*" matches "ls -la", "ls /tmp", etc.
                    let prefix = &pattern_arg[..pattern_arg.len() - 2];
                    command.starts_with(prefix)
                } else {
                    // Exact match
                    command == pattern_arg
                }
            }
            "Skill" => {
                let skill_name = tool_input
                    .get("skill")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if pattern_arg.ends_with(":*") {
                    let prefix = &pattern_arg[..pattern_arg.len() - 2];
                    skill_name.starts_with(prefix)
                } else {
                    skill_name == pattern_arg
                }
            }
            // Generic handling for other tools with parenthetical patterns
            _ => {
                // Try common argument fields
                let arg_value = tool_input
                    .get("pattern")
                    .or_else(|| tool_input.get("path"))
                    .or_else(|| tool_input.get("file_path"))
                    .or_else(|| tool_input.get("url"))
                    .or_else(|| tool_input.get("query"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if pattern_arg.ends_with(":*") {
                    let prefix = &pattern_arg[..pattern_arg.len() - 2];
                    arg_value.starts_with(prefix)
                } else {
                    arg_value == pattern_arg
                }
            }
        }
    } else {
        // Simple tool match: "WebFetch" matches tool_name == "WebFetch"
        pattern == tool_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn settings_with_allow(allow: Vec<&str>) -> ClaudeSettings {
        ClaudeSettings {
            permissions: Permissions {
                allow: allow.into_iter().map(String::from).collect(),
                deny: vec![],
            },
        }
    }

    // ============================================================================
    // Simple tool match tests
    // ============================================================================

    #[test]
    fn test_simple_tool_match() {
        let settings = settings_with_allow(vec!["WebFetch", "WebSearch"]);

        assert!(is_tool_allowed(&settings, "WebFetch", &json!({})));
        assert!(is_tool_allowed(&settings, "WebSearch", &json!({})));
        assert!(!is_tool_allowed(&settings, "Bash", &json!({})));
    }

    #[test]
    fn test_simple_tool_no_match() {
        let settings = settings_with_allow(vec!["Read"]);

        assert!(!is_tool_allowed(&settings, "Write", &json!({})));
        assert!(!is_tool_allowed(&settings, "Edit", &json!({})));
    }

    // ============================================================================
    // Skill pattern tests
    // ============================================================================

    #[test]
    fn test_skill_exact_match() {
        let settings = settings_with_allow(vec!["Skill(email)", "Skill(calendar)"]);

        assert!(is_tool_allowed(&settings, "Skill", &json!({"skill": "email"})));
        assert!(is_tool_allowed(&settings, "Skill", &json!({"skill": "calendar"})));
        assert!(!is_tool_allowed(&settings, "Skill", &json!({"skill": "commit"})));
    }

    #[test]
    fn test_skill_prefix_match() {
        let settings = settings_with_allow(vec!["Skill(frontend-:*)"]);

        assert!(is_tool_allowed(&settings, "Skill", &json!({"skill": "frontend-design"})));
        assert!(is_tool_allowed(&settings, "Skill", &json!({"skill": "frontend-test"})));
        assert!(!is_tool_allowed(&settings, "Skill", &json!({"skill": "backend"})));
    }

    // ============================================================================
    // Bash pattern tests
    // ============================================================================

    #[test]
    fn test_bash_prefix_match() {
        let settings = settings_with_allow(vec!["Bash(ls:*)", "Bash(pwd:*)"]);

        assert!(is_tool_allowed(&settings, "Bash", &json!({"command": "ls"})));
        assert!(is_tool_allowed(&settings, "Bash", &json!({"command": "ls -la"})));
        assert!(is_tool_allowed(&settings, "Bash", &json!({"command": "ls /tmp"})));
        assert!(is_tool_allowed(&settings, "Bash", &json!({"command": "pwd"})));
        assert!(!is_tool_allowed(&settings, "Bash", &json!({"command": "rm -rf /"})));
    }

    #[test]
    fn test_bash_exact_match() {
        let settings = settings_with_allow(vec!["Bash(uv init)"]);

        assert!(is_tool_allowed(&settings, "Bash", &json!({"command": "uv init"})));
        assert!(!is_tool_allowed(&settings, "Bash", &json!({"command": "uv init --help"})));
    }

    #[test]
    fn test_bash_complex_prefix() {
        let settings = settings_with_allow(vec!["Bash(npm run:*)", "Bash(cargo check:*)"]);

        assert!(is_tool_allowed(&settings, "Bash", &json!({"command": "npm run dev"})));
        assert!(is_tool_allowed(&settings, "Bash", &json!({"command": "npm run build"})));
        assert!(is_tool_allowed(&settings, "Bash", &json!({"command": "cargo check"})));
        assert!(is_tool_allowed(&settings, "Bash", &json!({"command": "cargo check --all-features"})));
        assert!(!is_tool_allowed(&settings, "Bash", &json!({"command": "npm install"})));
    }

    // ============================================================================
    // Edge cases
    // ============================================================================

    #[test]
    fn test_empty_allow_list() {
        let settings = settings_with_allow(vec![]);

        assert!(!is_tool_allowed(&settings, "WebFetch", &json!({})));
        assert!(!is_tool_allowed(&settings, "Bash", &json!({"command": "ls"})));
    }

    #[test]
    fn test_malformed_patterns() {
        // Pattern with ( but no ) should not match
        let settings = settings_with_allow(vec!["Bash(ls"]);

        assert!(!is_tool_allowed(&settings, "Bash", &json!({"command": "ls"})));
    }

    #[test]
    fn test_missing_command() {
        let settings = settings_with_allow(vec!["Bash(ls:*)"]);

        // Missing command field should not crash, just not match
        assert!(!is_tool_allowed(&settings, "Bash", &json!({})));
        assert!(!is_tool_allowed(&settings, "Bash", &json!({"other": "field"})));
    }
}
