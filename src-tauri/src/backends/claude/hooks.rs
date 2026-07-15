//! Permission logic for Claude CLI sessions.
//!
//! Historically this module also contained SDK hook builders. The SDK-backed backend has been
//! removed, so this file now only contains the permission decision logic used by the CLI backend.

use super::settings::{self, ClaudeSettings};

/// Result of evaluating whether a tool should be allowed to run.
#[derive(Debug, Clone, PartialEq)]
pub enum PermissionDecision {
    /// Tool is allowed to run, with the given reason.
    Allow(String),
    /// Tool is denied, with the given reason.
    Deny(String),
    /// Need to prompt the user for permission.
    PromptUser,
}

/// Determine whether a tool should be allowed based on mode, settings, and tool type.
///
/// Checks:
/// 1. Interactive tools -> Deny (require TTY input, can't be supported)
/// 2. Bypass mode -> Allow all tools
/// 3. Accept edits mode -> Allow known file-edit tools
/// 4. User settings (~/.claude/settings.json) -> Allow if explicitly allowed
/// 5. Known read-only tools -> Allow without prompting
/// 6. Everything else -> Prompt user
pub fn determine_permission(
    permission_mode: &str,
    tool_name: &str,
    tool_input: &serde_json::Value,
    user_settings: Option<&ClaudeSettings>,
) -> PermissionDecision {
    // 1. Interactive tools are never allowed - they require TTY input that we can't provide.
    if is_interactive_tool(tool_name) {
        return PermissionDecision::Deny(
            "Interactive tools requiring TTY input are not supported".to_string(),
        );
    }

    // 2. Check bypass mode - allows everything.
    if permission_mode == "bypassPermissions" {
        return PermissionDecision::Allow("Bypass mode - all tools allowed".to_string());
    }

    // 3. Check accept edits mode - allows known file-edit tools only.
    if permission_mode == "acceptEdits" && is_accept_edits_tool(tool_name) {
        return PermissionDecision::Allow(
            "AcceptEdits mode - file edit operations allowed".to_string(),
        );
    }

    // 4. Check user settings (~/.claude/settings.json).
    if let Some(settings) = user_settings {
        if settings::is_tool_allowed(settings, tool_name, tool_input) {
            return PermissionDecision::Allow("Allowed by user settings".to_string());
        }
    }

    // 5. Known read-only tools don't need prompting.
    if is_read_only_tool(tool_name) {
        return PermissionDecision::Allow("Read-only operation".to_string());
    }

    // 6. Unknown/protected tools require user prompt.
    PermissionDecision::PromptUser
}

/// Check if the tool requires permission prompting.
pub fn requires_permission(tool_name: &str) -> bool {
    !is_read_only_tool(tool_name)
}

/// Check if the tool is auto-allowed in acceptEdits mode.
fn is_accept_edits_tool(tool_name: &str) -> bool {
    matches!(tool_name, "Write" | "Edit" | "NotebookEdit")
}

/// Check if the tool is known read-only and safe to auto-allow.
fn is_read_only_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "Read"
            | "Glob"
            | "Grep"
            | "WebSearch"
            | "WebFetch"
            | "Task"
            | "TaskCreate"
            | "TaskUpdate"
            | "TaskList"
            | "TaskGet"
            | "TodoWrite"
            | "TodoRead"
    )
}

/// Check if the tool is interactive and requires TTY input.
pub fn is_interactive_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "AskUserQuestion" | "EnterPlanMode" | "ExitPlanMode"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unknown_tool_prompts_user() {
        let decision = determine_permission("default", "UnknownTool", &json!({}), None);
        assert_eq!(decision, PermissionDecision::PromptUser);
    }

    #[test]
    fn accept_edits_does_not_auto_allow_skill() {
        let decision = determine_permission(
            "acceptEdits",
            "Skill",
            &json!({"skill": "frontend-design"}),
            None,
        );
        assert_eq!(decision, PermissionDecision::PromptUser);
    }

    #[test]
    fn accept_edits_auto_allows_write_edit_and_notebook_edit() {
        for tool_name in ["Write", "Edit", "NotebookEdit"] {
            let decision = determine_permission("acceptEdits", tool_name, &json!({}), None);
            assert!(
                matches!(decision, PermissionDecision::Allow(_)),
                "expected Allow for {}, got {:?}",
                tool_name,
                decision
            );
        }
    }
}
