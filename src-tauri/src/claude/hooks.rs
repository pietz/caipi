//! Permission and tool hooks for the Claude agent session.
//!
//! This module contains all the hook logic for handling tool permissions,
//! including pre-tool-use and post-tool-use callbacks.

use crate::commands::chat::ChatEvent;
use claude_agent_sdk_rs::{
    HookCallback, HookContext, HookEvent, HookInput, HookJsonOutput, HookMatcher,
    PostToolUseHookInput, PreToolUseHookSpecificOutput, SyncHookJsonOutput, HookSpecificOutput,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::{oneshot, Mutex, Notify, RwLock};
use uuid::Uuid;

use super::agent::PermissionResponse;
use super::settings::{self, ClaudeSettings};
use super::tool_utils::extract_tool_target;

/// Type alias for permission channels - maps request ID to response sender
pub type PermissionChannels = Arc<Mutex<HashMap<String, oneshot::Sender<PermissionResponse>>>>;

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
            }
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
            }
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

/// Check if the permission mode allows the tool without prompting
pub fn check_mode_decision(mode: &str, tool_name: &str) -> Option<HookJsonOutput> {
    match mode {
        "bypassPermissions" => Some(allow_response("Bypass mode - all tools allowed")),
        "acceptEdits" if tool_name != "Bash" => {
            Some(allow_response("AcceptEdits mode - file operations allowed"))
        }
        _ => None,
    }
}

/// Check if the tool requires permission prompting
pub fn requires_permission(tool_name: &str) -> bool {
    matches!(tool_name, "Write" | "Edit" | "Bash" | "NotebookEdit" | "Skill")
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
    _session_id: String,
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
    let _ = app_handle.emit("claude:event", &ChatEvent::ToolStatusUpdate {
        tool_use_id: tool_use_id.clone(),
        status: "awaiting_permission".to_string(),
        permission_request_id: Some(request_id.clone()),
    });

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

    let _ = app_handle.emit("claude:event", &ChatEvent::ToolStatusUpdate {
        tool_use_id,
        status: status.to_string(),
        permission_request_id: None,
    });

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
    permission_mode_arc: Arc<RwLock<String>>,
    abort_flag: Arc<AtomicBool>,
    abort_notify: Arc<Notify>,
) -> HookCallback {
    // Load user settings once at hook creation
    let user_settings: Option<ClaudeSettings> = settings::load_user_settings();

    Arc::new(move |input: HookInput, tool_use_id: Option<String>, _ctx: HookContext| {
        let permission_channels = permission_channels.clone();
        let app_handle = app_handle.clone();
        let session_id = session_id.clone();
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
            let input_for_frontend = if tool_name.starts_with("Task") || tool_name.starts_with("Todo") {
                Some(tool_input.clone())
            } else {
                None
            };
            let target = extract_tool_target(&tool_name, &tool_input);

            let _ = app_handle.emit("claude:event", &ChatEvent::ToolStart {
                tool_use_id: tool_id.clone(),
                tool_type: tool_name.clone(),
                target,
                status: "pending".to_string(),
                input: input_for_frontend,
            });

            // 4. Check permission mode for auto-decisions
            let current_mode = permission_mode_arc.read().await.clone();
            if let Some(decision) = check_mode_decision(&current_mode, &tool_name) {
                // Emit status update: running (auto-approved)
                let _ = app_handle.emit("claude:event", &ChatEvent::ToolStatusUpdate {
                    tool_use_id: tool_id,
                    status: "running".to_string(),
                    permission_request_id: None,
                });
                return decision;
            }

            // 5. Check if allowed by user settings (~/.claude/settings.json)
            if let Some(ref settings) = user_settings {
                if settings::is_tool_allowed(settings, &tool_name, &tool_input) {
                    // Emit status update: running (allowed by user settings)
                    let _ = app_handle.emit("claude:event", &ChatEvent::ToolStatusUpdate {
                        tool_use_id: tool_id,
                        status: "running".to_string(),
                        permission_request_id: None,
                    });
                    return allow_response("Allowed by user settings");
                }
            }

            // 6. Check if this tool requires permission prompting
            if !requires_permission(&tool_name) {
                // Emit status update: running (no permission needed)
                let _ = app_handle.emit("claude:event", &ChatEvent::ToolStatusUpdate {
                    tool_use_id: tool_id,
                    status: "running".to_string(),
                    permission_request_id: None,
                });
                return allow_response("Read-only operation");
            }

            // 7. Prompt user for permission
            let request_id = Uuid::new_v4().to_string();
            prompt_user_permission(
                permission_channels,
                app_handle,
                session_id,
                tool_id,
                request_id,
                abort_notify,
                abort_flag,
            ).await
        })
    })
}

/// Build the post-tool-use hook callback to emit ToolEnd events
pub fn build_post_tool_use_hook(app_handle: AppHandle) -> HookCallback {
    Arc::new(move |input: HookInput, tool_use_id: Option<String>, _ctx: HookContext| {
        let app_handle = app_handle.clone();

        Box::pin(async move {
            if let HookInput::PostToolUse(PostToolUseHookInput { tool_response, .. }) = &input {
                // Determine if the tool errored by checking the response
                let is_error = tool_response.get("is_error")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let status = if is_error { "error" } else { "completed" };

                // Emit ToolEnd event immediately
                if let Some(id) = tool_use_id {
                    let _ = app_handle.emit("claude:event", &ChatEvent::ToolEnd {
                        id,
                        status: status.to_string(),
                    });
                }
            }

            // Don't modify anything, just return default
            HookJsonOutput::Sync(SyncHookJsonOutput::default())
        })
    })
}

/// Build the complete hooks map for an agent session
pub fn build_hooks(
    permission_channels: PermissionChannels,
    app_handle: AppHandle,
    session_id: String,
    permission_mode_arc: Arc<RwLock<String>>,
    abort_flag: Arc<AtomicBool>,
    abort_notify: Arc<Notify>,
) -> HashMap<HookEvent, Vec<HookMatcher>> {
    let pre_tool_use_hook = build_pre_tool_use_hook(
        permission_channels,
        app_handle.clone(),
        session_id,
        permission_mode_arc,
        abort_flag,
        abort_notify,
    );

    let post_tool_use_hook = build_post_tool_use_hook(app_handle);

    let mut hooks: HashMap<HookEvent, Vec<HookMatcher>> = HashMap::new();
    hooks.insert(
        HookEvent::PreToolUse,
        vec![HookMatcher::builder()
            .hooks(vec![pre_tool_use_hook])
            .build()]
    );
    hooks.insert(
        HookEvent::PostToolUse,
        vec![HookMatcher::builder()
            .hooks(vec![post_tool_use_hook])
            .build()]
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

    // ============================================================================
    // check_mode_decision tests
    // ============================================================================

    #[test]
    fn test_check_mode_decision_bypass() {
        // bypassPermissions mode returns allow for any tool
        let result = check_mode_decision("bypassPermissions", "Write");
        assert!(result.is_some());

        let result = check_mode_decision("bypassPermissions", "Bash");
        assert!(result.is_some());

        let result = check_mode_decision("bypassPermissions", "Read");
        assert!(result.is_some());
    }

    #[test]
    fn test_check_mode_decision_accept_edits_non_bash() {
        // acceptEdits mode returns allow for Write/Edit/etc (not Bash)
        let result = check_mode_decision("acceptEdits", "Write");
        assert!(result.is_some());

        let result = check_mode_decision("acceptEdits", "Edit");
        assert!(result.is_some());

        let result = check_mode_decision("acceptEdits", "NotebookEdit");
        assert!(result.is_some());
    }

    #[test]
    fn test_check_mode_decision_accept_edits_bash() {
        // acceptEdits mode returns None for Bash (requires prompt)
        let result = check_mode_decision("acceptEdits", "Bash");
        assert!(result.is_none());
    }

    #[test]
    fn test_check_mode_decision_default() {
        // default mode returns None (always prompt)
        let result = check_mode_decision("default", "Write");
        assert!(result.is_none());

        let result = check_mode_decision("default", "Bash");
        assert!(result.is_none());

        let result = check_mode_decision("default", "Read");
        assert!(result.is_none());
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
            if let Some(HookSpecificOutput::PreToolUse(pre_tool)) = sync_output.hook_specific_output {
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
            if let Some(HookSpecificOutput::PreToolUse(pre_tool)) = sync_output.hook_specific_output {
                assert_eq!(pre_tool.permission_decision, Some("allow".to_string()));
                assert_eq!(pre_tool.permission_decision_reason, Some("Test reason".to_string()));
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
            if let Some(HookSpecificOutput::PreToolUse(pre_tool)) = sync_output.hook_specific_output {
                assert_eq!(pre_tool.permission_decision, Some("deny".to_string()));
                assert_eq!(pre_tool.permission_decision_reason, Some("Test denial".to_string()));
                assert!(pre_tool.updated_input.is_none());
            } else {
                panic!("Expected PreToolUse hook specific output");
            }
        } else {
            panic!("Expected Sync hook output");
        }
    }

}
