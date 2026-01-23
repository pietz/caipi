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
use tauri::{AppHandle, Emitter};
use tokio::sync::{oneshot, Mutex, RwLock};
use uuid::Uuid;

use super::agent::PermissionResponse;

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
    matches!(tool_name, "Write" | "Edit" | "Bash" | "NotebookEdit")
}

/// Build a human-readable description for the permission prompt
pub fn build_permission_description(tool_name: &str, tool_input: &serde_json::Value) -> String {
    match tool_name {
        "Write" | "Edit" => {
            tool_input.get("file_path")
                .and_then(|v| v.as_str())
                .map(|p| format!("Modify file: {}", p))
                .unwrap_or_else(|| format!("Use tool: {}", tool_name))
        }
        "Bash" => {
            tool_input.get("command")
                .and_then(|v| v.as_str())
                .map(|cmd| {
                    if cmd.len() > 80 {
                        format!("Run command: {}...", &cmd[..77])
                    } else {
                        format!("Run command: {}", cmd)
                    }
                })
                .unwrap_or_else(|| "Run bash command".to_string())
        }
        _ => format!("Use tool: {}", tool_name),
    }
}

// ============================================================================
// Permission Prompting
// ============================================================================

/// Prompt the user for permission and await their response
pub async fn prompt_user_permission(
    permission_channels: PermissionChannels,
    app_handle: AppHandle,
    session_id: String,
    tool_name: String,
    tool_use_id: Option<String>,
    description: String,
) -> HookJsonOutput {
    let (tx, rx) = oneshot::channel();
    let request_id = Uuid::new_v4().to_string();

    // Store sender in the global permission channels
    {
        let mut channels = permission_channels.lock().await;
        channels.insert(request_id.clone(), tx);
    }

    // Emit permission request event to frontend
    let _ = app_handle.emit("claude:event", &ChatEvent::PermissionRequest {
        id: request_id,
        session_id,
        tool: tool_name,
        tool_use_id,
        description,
    });

    // Await user response
    match rx.await {
        Ok(response) if response.allowed => allow_response("User approved"),
        Ok(_) => deny_response("User denied"),
        Err(_) => deny_response("Permission request cancelled"),
    }
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
) -> HookCallback {
    Arc::new(move |input: HookInput, tool_use_id: Option<String>, _ctx: HookContext| {
        let permission_channels = permission_channels.clone();
        let app_handle = app_handle.clone();
        let session_id = session_id.clone();
        let permission_mode_arc = permission_mode_arc.clone();
        let abort_flag = abort_flag.clone();

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

            // 3. Check permission mode for auto-decisions
            let current_mode = permission_mode_arc.read().await.clone();
            if let Some(decision) = check_mode_decision(&current_mode, &tool_name) {
                return decision;
            }

            // 4. Check if this tool requires permission prompting
            if !requires_permission(&tool_name) {
                return allow_response("Read-only operation");
            }

            // 5. Build description and prompt user
            let description = build_permission_description(&tool_name, &tool_input);
            prompt_user_permission(
                permission_channels,
                app_handle,
                session_id,
                tool_name,
                tool_use_id,
                description,
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
) -> HashMap<HookEvent, Vec<HookMatcher>> {
    let pre_tool_use_hook = build_pre_tool_use_hook(
        permission_channels,
        app_handle.clone(),
        session_id,
        permission_mode_arc,
        abort_flag,
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

    #[test]
    fn test_requires_permission() {
        assert!(requires_permission("Write"));
        assert!(requires_permission("Edit"));
        assert!(requires_permission("Bash"));
        assert!(requires_permission("NotebookEdit"));
        assert!(!requires_permission("Read"));
        assert!(!requires_permission("Glob"));
    }

    #[test]
    fn test_build_permission_description() {
        let file_input = serde_json::json!({"file_path": "/test/file.rs"});
        assert_eq!(
            build_permission_description("Edit", &file_input),
            "Modify file: /test/file.rs"
        );

        let bash_input = serde_json::json!({"command": "ls -la"});
        assert_eq!(
            build_permission_description("Bash", &bash_input),
            "Run command: ls -la"
        );
    }
}
