//! Permission and tool hooks for the Claude agent session.
//!
//! This module contains all the hook logic for handling tool permissions,
//! including pre-tool-use and post-tool-use callbacks.

use crate::backends::{emit_chat_event, PermissionChannels, PermissionResponse};
use crate::commands::chat::ChatEvent;
use claude_agent_sdk_rs::{
    HookCallback, HookContext, HookEvent, HookInput, HookJsonOutput, HookMatcher,
    HookSpecificOutput, PreToolUseHookSpecificOutput, SyncHookJsonOutput,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;
use tokio::sync::{oneshot, Notify, RwLock};
use uuid::Uuid;

use super::settings::{self, ClaudeSettings};
use super::tool_utils::extract_tool_target;

// ============================================================================
// Permission Decision
// ============================================================================

/// Result of evaluating whether a tool should be allowed to run
#[derive(Debug, Clone, PartialEq)]
pub enum PermissionDecision {
    /// Tool is allowed to run, with the given reason
    Allow(String),
    /// Tool is denied, with the given reason (for future use by external callers)
    #[allow(dead_code)]
    Deny(String),
    /// Need to prompt the user for permission
    PromptUser,
}

/// Determine whether a tool should be allowed based on mode, settings, and tool type.
///
/// This is the core permission logic extracted for reuse. It checks:
/// 0. Interactive tools -> Deny (require TTY, can't be supported)
/// 1. Bypass mode -> Allow all tools
/// 2. Accept edits mode -> Allow file operations (not Bash)
/// 3. User settings -> Check if tool is explicitly allowed
/// 4. Read-only tools -> Allow without prompting
/// 5. Protected tools -> Require user prompt
pub fn determine_permission(
    permission_mode: &str,
    tool_name: &str,
    tool_input: &serde_json::Value,
    user_settings: Option<&ClaudeSettings>,
) -> PermissionDecision {
    // 0. Interactive tools are never allowed - they require TTY input
    // that we cannot provide when wrapping the CLI programmatically
    if is_interactive_tool(tool_name) {
        return PermissionDecision::Deny(
            "Interactive tools requiring TTY input are not supported".to_string(),
        );
    }

    // 1. Check bypass mode - allows everything
    if permission_mode == "bypassPermissions" {
        return PermissionDecision::Allow("Bypass mode - all tools allowed".to_string());
    }

    // 2. Check accept edits mode - allows file operations but not Bash
    if permission_mode == "acceptEdits" && tool_name != "Bash" {
        return PermissionDecision::Allow("AcceptEdits mode - file operations allowed".to_string());
    }

    // 3. Check user settings (~/.claude/settings.json)
    if let Some(settings) = user_settings {
        if settings::is_tool_allowed(settings, tool_name, tool_input) {
            return PermissionDecision::Allow("Allowed by user settings".to_string());
        }
    }

    // 4. Check if this is a read-only tool that doesn't need permission
    if !requires_permission(tool_name) {
        return PermissionDecision::Allow("Read-only operation".to_string());
    }

    // 5. Protected tools require user prompt
    PermissionDecision::PromptUser
}

// ============================================================================
// Hook Response Builders
// ============================================================================

/// Create an "allow" response for the pre-tool-use hook
pub fn allow_response(reason: &str) -> HookJsonOutput {
    HookJsonOutput::Sync(SyncHookJsonOutput {
        hook_specific_output: Some(HookSpecificOutput::PreToolUse(
            PreToolUseHookSpecificOutput {
                permission_decision: Some("allow".to_string()),
                permission_decision_reason: Some(reason.to_string()),
                updated_input: None,
            },
        )),
        ..Default::default()
    })
}

/// Create a "deny" response for the pre-tool-use hook
pub fn deny_response(reason: &str) -> HookJsonOutput {
    HookJsonOutput::Sync(SyncHookJsonOutput {
        hook_specific_output: Some(HookSpecificOutput::PreToolUse(
            PreToolUseHookSpecificOutput {
                permission_decision: Some("deny".to_string()),
                permission_decision_reason: Some(reason.to_string()),
                updated_input: None,
            },
        )),
        ..Default::default()
    })
}

// ============================================================================
// Decision Helpers
// ============================================================================

/// Check if the session has been aborted
pub fn check_abort_decision(abort_flag: &Arc<AtomicBool>) -> Option<HookJsonOutput> {
    if abort_flag.load(Ordering::SeqCst) {
        Some(deny_response("Session aborted"))
    } else {
        None
    }
}

/// Extract tool name and input from hook input
pub fn extract_tool_info(input: &HookInput) -> Option<(String, serde_json::Value)> {
    match input {
        HookInput::PreToolUse(pre_tool) => {
            Some((pre_tool.tool_name.clone(), pre_tool.tool_input.clone()))
        }
        _ => None,
    }
}

/// Check if the tool requires permission prompting
pub fn requires_permission(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "Write" | "Edit" | "Bash" | "NotebookEdit" | "Skill"
    )
}

/// Check if the tool is an interactive tool that requires TTY input.
/// These tools cannot be supported when wrapping the CLI programmatically
/// because they block waiting for terminal input that we cannot provide.
pub fn is_interactive_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "AskUserQuestion" | "EnterPlanMode" | "ExitPlanMode"
    )
}

// ============================================================================
// Permission Prompting
// ============================================================================

/// Outcome of waiting for permission
enum PermissionOutcome {
    Allowed,
    Denied,
    Cancelled,
    Timeout,
    Aborted,
}

/// Wait for permission response with abort and timeout support
async fn wait_for_permission(
    rx: oneshot::Receiver<PermissionResponse>,
    abort_notify: Arc<Notify>,
    abort_flag: Arc<AtomicBool>,
) -> PermissionOutcome {
    // Check abort flag before entering select! to avoid missing notifications
    // (Notify doesn't buffer, so if abort happened just before we call notified(), we'd miss it)
    if abort_flag.load(Ordering::SeqCst) {
        return PermissionOutcome::Aborted;
    }

    let timeout = tokio::time::sleep(Duration::from_secs(60));
    tokio::pin!(timeout);
    tokio::pin!(rx);

    tokio::select! {
        response = &mut rx => {
            match response {
                Ok(r) if r.allowed => PermissionOutcome::Allowed,
                Ok(_) => PermissionOutcome::Denied,
                Err(_) => PermissionOutcome::Cancelled,
            }
        }
        _ = &mut timeout => {
            PermissionOutcome::Timeout
        }
        _ = abort_notify.notified() => {
            // Double-check the flag in case of spurious wake
            if abort_flag.load(Ordering::SeqCst) {
                PermissionOutcome::Aborted
            } else {
                // Spurious wake, but we can't easily retry in select!
                // This is unlikely with Notify, so just treat as abort
                PermissionOutcome::Aborted
            }
        }
    }
}

/// Prompt the user for permission and await their response
pub async fn prompt_user_permission(
    permission_channels: PermissionChannels,
    app_handle: AppHandle,
    session_id: String,
    turn_id: Option<String>,
    tool_use_id: String,
    request_id: String,
    abort_notify: Arc<Notify>,
    abort_flag: Arc<AtomicBool>,
) -> HookJsonOutput {
    let (tx, rx) = oneshot::channel();

    // Store sender in the global permission channels
    {
        let mut channels = permission_channels.lock().await;
        channels.insert(request_id.clone(), tx);
    }

    // Emit status update: awaiting_permission
    let awaiting_permission = ChatEvent::ToolStatusUpdate {
        tool_use_id: tool_use_id.clone(),
        status: "awaiting_permission".to_string(),
        permission_request_id: Some(request_id.clone()),
    };
    emit_chat_event(
        &app_handle,
        Some(session_id.as_str()),
        turn_id.as_deref(),
        &awaiting_permission,
    );

    // Wait for permission response
    let outcome = wait_for_permission(rx, abort_notify, abort_flag).await;

    // Cleanup channel entry
    {
        let mut channels = permission_channels.lock().await;
        channels.remove(&request_id);
    }

    // Emit status update based on outcome
    let (status, result) = match outcome {
        PermissionOutcome::Allowed => ("running", allow_response("User approved")),
        PermissionOutcome::Denied => ("denied", deny_response("User denied")),
        PermissionOutcome::Cancelled => ("denied", deny_response("Permission request cancelled")),
        PermissionOutcome::Timeout => ("denied", deny_response("Permission request timed out")),
        PermissionOutcome::Aborted => ("denied", deny_response("Session aborted")),
    };

    let final_status_event = ChatEvent::ToolStatusUpdate {
        tool_use_id,
        status: status.to_string(),
        permission_request_id: None,
    };
    emit_chat_event(
        &app_handle,
        Some(session_id.as_str()),
        turn_id.as_deref(),
        &final_status_event,
    );

    result
}

// ============================================================================
// Hook Builders
// ============================================================================

/// Build the pre-tool-use hook callback
pub fn build_pre_tool_use_hook(
    permission_channels: PermissionChannels,
    app_handle: AppHandle,
    session_id: String,
    turn_id: Option<String>,
    permission_mode_arc: Arc<RwLock<String>>,
    abort_flag: Arc<AtomicBool>,
    abort_notify: Arc<Notify>,
) -> HookCallback {
    // Load user settings once at hook creation
    let user_settings: Option<ClaudeSettings> = settings::load_user_settings();

    Arc::new(
        move |input: HookInput, tool_use_id: Option<String>, _ctx: HookContext| {
            let permission_channels = permission_channels.clone();
            let app_handle = app_handle.clone();
            let session_id = session_id.clone();
            let turn_id = turn_id.clone();
            let permission_mode_arc = permission_mode_arc.clone();
            let abort_flag = abort_flag.clone();
            let abort_notify = abort_notify.clone();
            let user_settings = user_settings.clone();

            Box::pin(async move {
                // 1. Check if abort was requested
                if let Some(deny) = check_abort_decision(&abort_flag) {
                    return deny;
                }

                // 2. Extract tool info (only handle PreToolUse events)
                let (tool_name, tool_input) = match extract_tool_info(&input) {
                    Some(info) => info,
                    None => return HookJsonOutput::Sync(SyncHookJsonOutput::default()),
                };

                // Get tool_use_id or generate a fallback (should always have one from SDK)
                let tool_id = tool_use_id.unwrap_or_else(|| Uuid::new_v4().to_string());

                // 3. Emit ToolStart with pending status
                // Include input for task/todo tools so frontend can update task list
                let input_for_frontend =
                    if tool_name.starts_with("Task") || tool_name.starts_with("Todo") {
                        Some(tool_input.clone())
                    } else {
                        None
                    };
                let target = extract_tool_target(&tool_name, &tool_input);

                let tool_start = ChatEvent::ToolStart {
                    tool_use_id: tool_id.clone(),
                    tool_type: tool_name.clone(),
                    target,
                    status: "pending".to_string(),
                    input: input_for_frontend,
                };
                emit_chat_event(
                    &app_handle,
                    Some(session_id.as_str()),
                    turn_id.as_deref(),
                    &tool_start,
                );

                // 4. Determine permission using the reusable function
                let current_mode = permission_mode_arc.read().await.clone();
                let decision = determine_permission(
                    &current_mode,
                    &tool_name,
                    &tool_input,
                    user_settings.as_ref(),
                );

                match decision {
                    PermissionDecision::Allow(reason) => {
                        // Emit status update: running (auto-approved)
                        let running_status = ChatEvent::ToolStatusUpdate {
                            tool_use_id: tool_id,
                            status: "running".to_string(),
                            permission_request_id: None,
                        };
                        emit_chat_event(
                            &app_handle,
                            Some(session_id.as_str()),
                            turn_id.as_deref(),
                            &running_status,
                        );
                        return allow_response(&reason);
                    }
                    PermissionDecision::Deny(reason) => {
                        // Emit status update: denied
                        let denied_status = ChatEvent::ToolStatusUpdate {
                            tool_use_id: tool_id,
                            status: "denied".to_string(),
                            permission_request_id: None,
                        };
                        emit_chat_event(
                            &app_handle,
                            Some(session_id.as_str()),
                            turn_id.as_deref(),
                            &denied_status,
                        );
                        return deny_response(&reason);
                    }
                    PermissionDecision::PromptUser => {
                        // Continue to prompt user below
                    }
                }

                // 5. Prompt user for permission
                let request_id = Uuid::new_v4().to_string();
                prompt_user_permission(
                    permission_channels,
                    app_handle,
                    session_id,
                    turn_id,
                    tool_id,
                    request_id,
                    abort_notify,
                    abort_flag,
                )
                .await
            })
        },
    )
}

/// Build the complete hooks map for an agent session
pub fn build_hooks(
    permission_channels: PermissionChannels,
    app_handle: AppHandle,
    session_id: String,
    turn_id: Option<String>,
    permission_mode_arc: Arc<RwLock<String>>,
    abort_flag: Arc<AtomicBool>,
    abort_notify: Arc<Notify>,
) -> HashMap<HookEvent, Vec<HookMatcher>> {
    let pre_tool_use_hook = build_pre_tool_use_hook(
        permission_channels,
        app_handle,
        session_id,
        turn_id,
        permission_mode_arc,
        abort_flag,
        abort_notify,
    );

    let mut hooks: HashMap<HookEvent, Vec<HookMatcher>> = HashMap::new();
    hooks.insert(
        HookEvent::PreToolUse,
        vec![HookMatcher::builder()
            .hooks(vec![pre_tool_use_hook])
            .build()],
    );

    hooks
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_requires_permission() {
        assert!(requires_permission("Write"));
        assert!(requires_permission("Edit"));
        assert!(requires_permission("Bash"));
        assert!(requires_permission("NotebookEdit"));
        assert!(requires_permission("Skill"));
        assert!(!requires_permission("Read"));
        assert!(!requires_permission("Glob"));
    }

    #[test]
    fn test_is_interactive_tool() {
        // Interactive tools that require TTY
        assert!(is_interactive_tool("AskUserQuestion"));
        assert!(is_interactive_tool("EnterPlanMode"));
        assert!(is_interactive_tool("ExitPlanMode"));
        // Non-interactive tools
        assert!(!is_interactive_tool("Read"));
        assert!(!is_interactive_tool("Write"));
        assert!(!is_interactive_tool("Bash"));
    }

    #[test]
    fn test_determine_permission_denies_interactive_tools() {
        // Interactive tools should always be denied, even in bypass mode
        let input = json!({});

        let result = determine_permission("bypassPermissions", "AskUserQuestion", &input, None);
        assert!(matches!(result, PermissionDecision::Deny(_)));

        let result = determine_permission("default", "EnterPlanMode", &input, None);
        assert!(matches!(result, PermissionDecision::Deny(_)));

        let result = determine_permission("acceptEdits", "ExitPlanMode", &input, None);
        assert!(matches!(result, PermissionDecision::Deny(_)));
    }

    // ============================================================================
    // determine_permission tests
    // ============================================================================

    #[test]
    fn test_determine_permission_bypass_mode() {
        // Bypass mode allows all tools
        let input = json!({});

        let result = determine_permission("bypassPermissions", "Write", &input, None);
        assert!(matches!(result, PermissionDecision::Allow(_)));

        let result = determine_permission("bypassPermissions", "Bash", &input, None);
        assert!(matches!(result, PermissionDecision::Allow(_)));

        let result = determine_permission("bypassPermissions", "Read", &input, None);
        assert!(matches!(result, PermissionDecision::Allow(_)));
    }

    #[test]
    fn test_determine_permission_accept_edits_mode() {
        // AcceptEdits mode allows file operations but not Bash
        let input = json!({});

        let result = determine_permission("acceptEdits", "Write", &input, None);
        assert!(matches!(result, PermissionDecision::Allow(_)));

        let result = determine_permission("acceptEdits", "Edit", &input, None);
        assert!(matches!(result, PermissionDecision::Allow(_)));

        let result = determine_permission("acceptEdits", "NotebookEdit", &input, None);
        assert!(matches!(result, PermissionDecision::Allow(_)));

        // Bash requires prompt in acceptEdits mode
        let result = determine_permission("acceptEdits", "Bash", &input, None);
        assert!(matches!(result, PermissionDecision::PromptUser));
    }

    #[test]
    fn test_determine_permission_default_mode_read_only() {
        // Default mode allows read-only tools without prompting
        let input = json!({});

        let result = determine_permission("default", "Read", &input, None);
        assert!(matches!(result, PermissionDecision::Allow(_)));

        let result = determine_permission("default", "Glob", &input, None);
        assert!(matches!(result, PermissionDecision::Allow(_)));

        let result = determine_permission("default", "Grep", &input, None);
        assert!(matches!(result, PermissionDecision::Allow(_)));

        let result = determine_permission("default", "WebFetch", &input, None);
        assert!(matches!(result, PermissionDecision::Allow(_)));
    }

    #[test]
    fn test_determine_permission_default_mode_protected() {
        // Default mode requires prompt for protected tools
        let input = json!({});

        let result = determine_permission("default", "Write", &input, None);
        assert!(matches!(result, PermissionDecision::PromptUser));

        let result = determine_permission("default", "Edit", &input, None);
        assert!(matches!(result, PermissionDecision::PromptUser));

        let result = determine_permission("default", "Bash", &input, None);
        assert!(matches!(result, PermissionDecision::PromptUser));

        let result = determine_permission("default", "NotebookEdit", &input, None);
        assert!(matches!(result, PermissionDecision::PromptUser));

        let result = determine_permission("default", "Skill", &input, None);
        assert!(matches!(result, PermissionDecision::PromptUser));
    }

    // ============================================================================
    // extract_tool_target tests
    // ============================================================================

    #[test]
    fn test_extract_tool_target_read() {
        // Read tool extracts file_path
        let input = json!({
            "file_path": "/path/to/file.rs"
        });
        let result = extract_tool_target("Read", &input);
        assert_eq!(result, "/path/to/file.rs");
    }

    #[test]
    fn test_extract_tool_target_bash() {
        // Bash tool prefers description over command
        let with_description = json!({
            "command": "git commit -m 'Fix bug'",
            "description": "Create commit with fix message"
        });
        let result = extract_tool_target("Bash", &with_description);
        assert_eq!(result, "Create commit with fix message");

        // Falls back to command if no description
        let short_cmd = json!({
            "command": "ls -la"
        });
        let result = extract_tool_target("Bash", &short_cmd);
        assert_eq!(result, "ls -la");

        // Long descriptions are passed through (CSS handles truncation)
        let long_desc = "This is a very long description that would have been truncated before";
        let long_desc_input = json!({
            "command": "some command",
            "description": long_desc
        });
        let result = extract_tool_target("Bash", &long_desc_input);
        assert_eq!(result, long_desc);

        // Long commands are passed through (CSS handles truncation)
        let long_cmd = "this is a very long command that would have been truncated before";
        let long_cmd_input = json!({
            "command": long_cmd
        });
        let result = extract_tool_target("Bash", &long_cmd_input);
        assert_eq!(result, long_cmd);
    }

    #[test]
    fn test_extract_tool_target_glob() {
        // Glob tool extracts pattern
        let input = json!({
            "pattern": "**/*.rs"
        });
        let result = extract_tool_target("Glob", &input);
        assert_eq!(result, "**/*.rs");
    }

    #[test]
    fn test_extract_tool_target_unknown() {
        // Unknown tool tries common fields then falls back to name
        let input_with_field = json!({
            "file_path": "/some/path.txt"
        });
        let result = extract_tool_target("CustomTool", &input_with_field);
        assert_eq!(result, "CustomTool: /some/path.txt");

        // No common fields - falls back to tool name
        let input_no_field = json!({
            "custom_param": "value"
        });
        let result = extract_tool_target("CustomTool", &input_no_field);
        assert_eq!(result, "CustomTool");
    }

    #[test]
    fn test_extract_tool_target_write() {
        // Write tool extracts file_path
        let input = json!({
            "file_path": "/output/data.json",
            "content": "some content"
        });
        let result = extract_tool_target("Write", &input);
        assert_eq!(result, "/output/data.json");
    }

    #[test]
    fn test_extract_tool_target_edit() {
        // Edit tool extracts file_path
        let input = json!({
            "file_path": "/src/main.rs",
            "old_string": "old",
            "new_string": "new"
        });
        let result = extract_tool_target("Edit", &input);
        assert_eq!(result, "/src/main.rs");
    }

    #[test]
    fn test_extract_tool_target_grep() {
        // Grep tool extracts pattern
        let input = json!({
            "pattern": "fn main"
        });
        let result = extract_tool_target("Grep", &input);
        assert_eq!(result, "fn main");
    }

    #[test]
    fn test_extract_tool_target_web_search() {
        // WebSearch tool extracts query
        let input = json!({
            "query": "rust async programming"
        });
        let result = extract_tool_target("WebSearch", &input);
        assert_eq!(result, "rust async programming");
    }

    #[test]
    fn test_extract_tool_target_skill() {
        // Skill tool extracts skill name
        let input = json!({
            "skill": "commit",
            "args": "-m 'message'"
        });
        let result = extract_tool_target("Skill", &input);
        assert_eq!(result, "commit");
    }

    #[test]
    fn test_extract_tool_target_notebook_edit() {
        // NotebookEdit tool extracts notebook_path
        let input = json!({
            "notebook_path": "/notebooks/analysis.ipynb",
            "new_source": "print('hello')"
        });
        let result = extract_tool_target("NotebookEdit", &input);
        assert_eq!(result, "/notebooks/analysis.ipynb");
    }

    // ============================================================================
    // check_abort_decision tests
    // ============================================================================

    #[test]
    fn test_check_abort_decision_not_aborted() {
        // Returns None when flag is false
        let abort_flag = Arc::new(AtomicBool::new(false));
        let result = check_abort_decision(&abort_flag);
        assert!(result.is_none());
    }

    #[test]
    fn test_check_abort_decision_aborted() {
        // Returns deny when flag is true
        let abort_flag = Arc::new(AtomicBool::new(true));
        let result = check_abort_decision(&abort_flag);
        assert!(result.is_some());

        // Verify it's a deny response by checking the structure
        if let Some(HookJsonOutput::Sync(sync_output)) = result {
            if let Some(HookSpecificOutput::PreToolUse(pre_tool)) = sync_output.hook_specific_output
            {
                assert_eq!(pre_tool.permission_decision, Some("deny".to_string()));
                assert!(pre_tool.permission_decision_reason.is_some());
            } else {
                panic!("Expected PreToolUse hook specific output");
            }
        } else {
            panic!("Expected Sync hook output");
        }
    }

    // ============================================================================
    // Response builder tests
    // ============================================================================

    #[test]
    fn test_allow_response() {
        let result = allow_response("Test reason");

        if let HookJsonOutput::Sync(sync_output) = result {
            if let Some(HookSpecificOutput::PreToolUse(pre_tool)) = sync_output.hook_specific_output
            {
                assert_eq!(pre_tool.permission_decision, Some("allow".to_string()));
                assert_eq!(
                    pre_tool.permission_decision_reason,
                    Some("Test reason".to_string())
                );
                assert!(pre_tool.updated_input.is_none());
            } else {
                panic!("Expected PreToolUse hook specific output");
            }
        } else {
            panic!("Expected Sync hook output");
        }
    }

    #[test]
    fn test_deny_response() {
        let result = deny_response("Test denial");

        if let HookJsonOutput::Sync(sync_output) = result {
            if let Some(HookSpecificOutput::PreToolUse(pre_tool)) = sync_output.hook_specific_output
            {
                assert_eq!(pre_tool.permission_decision, Some("deny".to_string()));
                assert_eq!(
                    pre_tool.permission_decision_reason,
                    Some("Test denial".to_string())
                );
                assert!(pre_tool.updated_input.is_none());
            } else {
                panic!("Expected PreToolUse hook specific output");
            }
        } else {
            panic!("Expected Sync hook output");
        }
    }
}
