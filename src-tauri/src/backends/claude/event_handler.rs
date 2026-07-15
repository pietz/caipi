//! Event handling for the Claude CLI backend.
//!
//! Contains the event dispatch loop and individual event type handlers
//! extracted from `adapter.rs`. All functions are associated functions on
//! `CliSession` (no `&self`) so they can be called from spawned tasks that
//! hold `Arc` clones of the required state.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::{Mutex, Notify, RwLock};
use uuid::Uuid;

use crate::backends::emit_chat_event;
use crate::backends::types::BackendError;
use crate::backends::types::{ChatEvent, Message};
use crate::backends::PermissionChannels;

use super::adapter::CliSession;
use super::cli_protocol::{
    AssistantEvent, CliEvent, ContentBlock, HookResponseData, IncomingControlRequest,
    OutgoingControlResponse, OutgoingHookSpecificOutput, OutgoingResponsePayload, ResultEvent,
    SystemEvent, UsageInfo,
};
use super::hooks::{determine_permission, PermissionDecision};
use super::settings::ClaudeSettings;
use super::tool_utils::extract_tool_target;

#[derive(Clone, Copy)]
struct EventContext<'a> {
    app_handle: &'a AppHandle,
    session_id: &'a str,
    turn_id: Option<&'a str>,
}

impl EventContext<'_> {
    fn emit(&self, event: &ChatEvent) {
        emit_chat_event(self.app_handle, Some(self.session_id), self.turn_id, event);
    }
}

#[derive(Clone, Copy)]
struct ControlContext<'a> {
    event: EventContext<'a>,
    permission_mode: &'a Arc<RwLock<String>>,
    user_settings: Option<&'a ClaudeSettings>,
    permission_channels: &'a PermissionChannels,
    stdin_writer: &'a Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    abort_flag: &'a Arc<AtomicBool>,
    abort_notify: &'a Arc<Notify>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum HookCallbackAction {
    Ignore,
    PreToolUse,
    PostToolUse,
    UnknownHook(String),
    MissingInput,
}

impl CliSession {
    /// Compute context usage for UI from assistant usage.
    /// This tracks effective input-side context load for the current call.
    pub(super) fn context_tokens_from_usage(usage: &UsageInfo) -> u64 {
        usage.input_tokens + usage.cache_read_input_tokens + usage.cache_creation_input_tokens
    }

    fn ack_hook_continue(request_id: String, hook_event_name: &str) -> OutgoingControlResponse {
        OutgoingControlResponse {
            msg_type: "control_response".to_string(),
            response: OutgoingResponsePayload {
                subtype: "success".to_string(),
                request_id,
                response: Some(HookResponseData {
                    continue_: true,
                    hook_specific_output: OutgoingHookSpecificOutput {
                        hook_event_name: hook_event_name.to_string(),
                        permission_decision: "allow".to_string(),
                        permission_decision_reason: Some("Acknowledged".to_string()),
                        updated_input: None,
                    },
                }),
            },
        }
    }

    fn is_error_result_subtype(subtype: &str) -> bool {
        let subtype = subtype.to_ascii_lowercase();
        subtype == "error" || subtype.contains("error") || subtype.contains("fail")
    }

    fn classify_hook_callback(request: &IncomingControlRequest) -> HookCallbackAction {
        if request.request.subtype != "hook_callback" {
            return HookCallbackAction::Ignore;
        }

        match request
            .request
            .input
            .as_ref()
            .map(|input| input.hook_event_name.as_str())
        {
            Some("PreToolUse") => HookCallbackAction::PreToolUse,
            Some("PostToolUse") => HookCallbackAction::PostToolUse,
            Some(other) => HookCallbackAction::UnknownHook(other.to_string()),
            None => HookCallbackAction::MissingInput,
        }
    }

    fn pretool_prompt_resolution(allowed: bool) -> (&'static str, &'static str) {
        if allowed {
            ("running", "User approved")
        } else {
            ("denied", "User denied")
        }
    }

    /// Send a control response to CLI stdin.
    pub(super) async fn send_control_response(
        stdin_writer: &Arc<Mutex<Option<tokio::process::ChildStdin>>>,
        response: OutgoingControlResponse,
    ) -> Result<(), BackendError> {
        let mut stdin_guard = stdin_writer.lock().await;
        if let Some(ref mut stdin) = *stdin_guard {
            use tokio::io::AsyncWriteExt;

            let json_line = serde_json::to_string(&response).map_err(|e| BackendError {
                message: format!("Failed to serialize control response: {}", e),
                recoverable: false,
            })?;

            stdin
                .write_all(json_line.as_bytes())
                .await
                .map_err(|e| BackendError {
                    message: format!("Failed to write control response: {}", e),
                    recoverable: false,
                })?;
            stdin.write_all(b"\n").await.map_err(|e| BackendError {
                message: format!("Failed to write newline: {}", e),
                recoverable: false,
            })?;
            stdin.flush().await.map_err(|e| BackendError {
                message: format!("Failed to flush stdin: {}", e),
                recoverable: false,
            })?;
            Ok(())
        } else {
            Err(BackendError {
                message: "CLI stdin not available".to_string(),
                recoverable: false,
            })
        }
    }

    /// Handle a CLI event.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn handle_event(
        event: CliEvent,
        app_handle: &AppHandle,
        turn_id: Option<&str>,
        permission_mode: &Arc<RwLock<String>>,
        user_settings: Option<&ClaudeSettings>,
        permission_channels: &PermissionChannels,
        stdin_writer: &Arc<Mutex<Option<tokio::process::ChildStdin>>>,
        abort_flag: &Arc<AtomicBool>,
        abort_notify: &Arc<Notify>,
        session_id: &str,
        active_tools: &mut HashMap<String, String>,
        messages: &Arc<RwLock<Vec<Message>>>,
        cli_session_id: &Arc<RwLock<Option<String>>>,
        in_flight: &Arc<AtomicBool>,
        current_turn_id: &Arc<RwLock<Option<String>>>,
    ) {
        let event_ctx = EventContext {
            app_handle,
            session_id,
            turn_id,
        };

        match event {
            CliEvent::System(system) => {
                Self::handle_system_event(system, event_ctx, cli_session_id).await;
            }
            CliEvent::Assistant(assistant) => {
                Self::handle_assistant_event(assistant, event_ctx, active_tools, messages).await;
            }
            CliEvent::User(_user) => {
                Self::handle_user_event(_user, event_ctx, active_tools).await;
            }
            CliEvent::Result(result) => {
                Self::handle_result_event(result, event_ctx, in_flight, current_turn_id).await;
            }
            CliEvent::ControlRequest(request) => {
                let control_ctx = ControlContext {
                    event: event_ctx,
                    permission_mode,
                    user_settings,
                    permission_channels,
                    stdin_writer,
                    abort_flag,
                    abort_notify,
                };
                Self::handle_control_request(request, control_ctx, active_tools).await;
            }
            CliEvent::ControlResponse(_ack) => {
                // Acknowledgment of our control response - nothing to do
            }
            CliEvent::Unknown => {
                // Unknown event type from a newer CLI version - skip gracefully
            }
        }
    }

    /// Handle system events (init, health_check).
    async fn handle_system_event(
        event: SystemEvent,
        ctx: EventContext<'_>,
        cli_session_id: &Arc<RwLock<Option<String>>>,
    ) {
        if event.subtype == "init" {
            // Capture CLI session ID for message correlation
            if let Some(sid) = event.session_id {
                log::info!("CLI session initialized: id={}", sid);
                *cli_session_id.write().await = Some(sid);
            }

            // Parse apiKeySource from init event data
            let api_key_source = event
                .data
                .get("apiKeySource")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            let auth_type = match api_key_source {
                "none" => "Claude AI Subscription",
                "environment" | "settings" => "Anthropic API Key",
                _ => "Unknown",
            }
            .to_string();

            let session_init = ChatEvent::SessionInit { auth_type };
            ctx.emit(&session_init);
        }
    }

    /// Handle assistant events (messages with content blocks).
    /// Note: ToolStart is now emitted from hook callbacks, not from tool_use blocks.
    async fn handle_assistant_event(
        event: AssistantEvent,
        ctx: EventContext<'_>,
        active_tools: &mut HashMap<String, String>,
        messages: &Arc<RwLock<Vec<Message>>>,
    ) {
        if let Some(usage) = &event.message.usage {
            let total_tokens = Self::context_tokens_from_usage(usage);
            let token_usage = ChatEvent::TokenUsage {
                total_tokens,
                context_tokens: None,
                context_window: None,
            };
            ctx.emit(&token_usage);
        }

        for block in event.message.content {
            match block {
                ContentBlock::Text(text_block) => {
                    let text_event = ChatEvent::Text {
                        content: text_block.text.clone(),
                    };
                    ctx.emit(&text_event);

                    // Store message
                    let mut msgs = messages.write().await;
                    msgs.push(Message::new("assistant", text_block.text));
                }
                ContentBlock::Thinking(thinking_block) => {
                    let thinking_id = Uuid::new_v4().to_string();
                    let thinking_start = ChatEvent::ThinkingStart {
                        thinking_id: thinking_id.clone(),
                        content: thinking_block.thinking,
                        input: None,
                    };
                    ctx.emit(&thinking_start);
                    let thinking_end = ChatEvent::ThinkingEnd { thinking_id };
                    ctx.emit(&thinking_end);
                }
                ContentBlock::ToolUse(tool_use) => {
                    // Track the tool for ToolEnd matching
                    // ToolStart is emitted from the PreToolUse hook callback, not here
                    active_tools.insert(tool_use.id.clone(), tool_use.name.clone());
                }
                ContentBlock::ToolResult(tool_result) => {
                    // Tool completed. Guard on active_tools to avoid duplicate ToolEnd
                    // emissions if both User and Assistant streams include tool_result.
                    if active_tools.remove(&tool_result.tool_use_id).is_some() {
                        let status = if tool_result.is_error {
                            "error"
                        } else {
                            "completed"
                        };
                        let tool_end = ChatEvent::ToolEnd {
                            id: tool_result.tool_use_id.clone(),
                            status: status.to_string(),
                            output: Some(tool_result.content.clone()),
                        };
                        ctx.emit(&tool_end);
                    }
                }
                ContentBlock::InputJsonDelta(_) => {
                    // Streaming delta - we can ignore for now since we get the complete input later
                }
                ContentBlock::Unknown => {
                    // Unknown content block type from a newer CLI version - skip gracefully
                }
            }
        }
        // Token usage is emitted from assistant usage (per call) to represent
        // context usage, not cumulative session totals.
    }

    /// Handle user events (tool results).
    async fn handle_user_event(
        event: super::cli_protocol::UserEvent,
        ctx: EventContext<'_>,
        active_tools: &mut HashMap<String, String>,
    ) {
        if let Some(message) = event.extra.get("message") {
            if let Some(content_array) = message.get("content").and_then(|c| c.as_array()) {
                for item in content_array {
                    if item.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                        if let Some(tool_use_id) =
                            item.get("tool_use_id").and_then(|id| id.as_str())
                        {
                            // Emit once per tool ID. Assistant blocks may also contain tool_result
                            // in some protocol variants.
                            if active_tools.remove(tool_use_id).is_some() {
                                let is_error = item
                                    .get("is_error")
                                    .and_then(|e| e.as_bool())
                                    .unwrap_or(false);
                                let status = if is_error { "error" } else { "completed" };
                                let tool_end = ChatEvent::ToolEnd {
                                    id: tool_use_id.to_string(),
                                    status: status.to_string(),
                                    output: item.get("content").cloned(),
                                };
                                ctx.emit(&tool_end);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Handle a control request from the CLI (hook callbacks).
    /// This is where ToolStart is emitted and permissions are determined.
    async fn handle_control_request(
        request: IncomingControlRequest,
        ctx: ControlContext<'_>,
        active_tools: &mut HashMap<String, String>,
    ) {
        let response = match Self::classify_hook_callback(&request) {
            HookCallbackAction::Ignore => return,
            HookCallbackAction::PreToolUse => {
                if let Some(input) = request.request.input.as_ref() {
                    Self::handle_pretool_hook(&request, input, ctx, active_tools).await;
                    return;
                }

                log::debug!("PreToolUse hook callback missing input; sending safe continue ack");
                Self::ack_hook_continue(request.request_id.clone(), "UnknownHook")
            }
            HookCallbackAction::PostToolUse => {
                // Acknowledge PostToolUse.
                // Tool completion is emitted from ToolResult blocks so we preserve
                // the real final status (including errors) and avoid duplicate ToolEnd events.
                OutgoingControlResponse::ack_posttool(request.request_id.clone())
            }
            HookCallbackAction::UnknownHook(hook_event_name) => {
                log::debug!(
                    "Unknown hook callback '{}'; sending safe continue ack",
                    hook_event_name
                );
                Self::ack_hook_continue(request.request_id.clone(), &hook_event_name)
            }
            HookCallbackAction::MissingInput => {
                log::debug!("hook_callback missing input; sending safe continue ack");
                Self::ack_hook_continue(request.request_id.clone(), "UnknownHook")
            }
        };

        if let Err(e) = Self::send_control_response(ctx.stdin_writer, response).await {
            log::error!("Failed to send hook callback ack: {}", e);
        }
    }

    /// Handle a PreToolUse hook callback - emit ToolStart and determine permission.
    async fn handle_pretool_hook(
        request: &IncomingControlRequest,
        input: &super::cli_protocol::HookCallbackInput,
        ctx: ControlContext<'_>,
        active_tools: &mut HashMap<String, String>,
    ) {
        let tool_name = input.tool_name.clone().unwrap_or_default();
        let tool_input = input.tool_input.clone().unwrap_or(serde_json::json!({}));
        let tool_use_id = request
            .request
            .tool_use_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        // Track this tool
        active_tools.insert(tool_use_id.clone(), tool_name.clone());

        // Extract target for display
        let target = extract_tool_target(&tool_name, &tool_input);

        // Emit ToolStart with pending status
        let input_for_frontend = if tool_name.starts_with("Task") || tool_name.starts_with("Todo") {
            Some(tool_input.clone())
        } else {
            None
        };

        let tool_start = ChatEvent::ToolStart {
            tool_use_id: tool_use_id.clone(),
            tool_type: tool_name.clone(),
            target,
            status: "pending".to_string(),
            input: input_for_frontend,
        };
        ctx.event.emit(&tool_start);

        // Check abort first
        if ctx.abort_flag.load(Ordering::SeqCst) {
            // Remove from active_tools since tool won't run
            active_tools.remove(&tool_use_id);
            let response = OutgoingControlResponse::deny_pretool(
                request.request_id.clone(),
                "Session aborted",
            );
            if let Err(e) = Self::send_control_response(ctx.stdin_writer, response).await {
                log::error!("Failed to send abort response: {}", e);
            }
            return;
        }

        // Determine permission
        let current_mode = ctx.permission_mode.read().await.clone();
        let decision =
            determine_permission(&current_mode, &tool_name, &tool_input, ctx.user_settings);
        log::debug!("Permission: tool={}, decision={:?}", tool_name, decision);

        match decision {
            PermissionDecision::Allow(reason) => {
                // Auto-approved - emit running status and send allow response
                let running_status = ChatEvent::ToolStatusUpdate {
                    tool_use_id: tool_use_id.clone(),
                    status: "running".to_string(),
                    permission_request_id: None,
                };
                ctx.event.emit(&running_status);

                let response =
                    OutgoingControlResponse::allow_pretool(request.request_id.clone(), &reason);
                if let Err(e) = Self::send_control_response(ctx.stdin_writer, response).await {
                    log::error!("Failed to send allow response: {}", e);
                }
            }
            PermissionDecision::Deny(reason) => {
                // Denied - remove from active_tools, emit denied status, send deny response
                active_tools.remove(&tool_use_id);
                let denied_status = ChatEvent::ToolStatusUpdate {
                    tool_use_id: tool_use_id.clone(),
                    status: "denied".to_string(),
                    permission_request_id: None,
                };
                ctx.event.emit(&denied_status);

                let response =
                    OutgoingControlResponse::deny_pretool(request.request_id.clone(), &reason);
                if let Err(e) = Self::send_control_response(ctx.stdin_writer, response).await {
                    log::error!("Failed to send deny response: {}", e);
                }
            }
            PermissionDecision::PromptUser => {
                // Need to prompt user - set up permission channel and wait
                let permission_request_id = Uuid::new_v4().to_string();

                // Emit awaiting_permission status
                let awaiting_permission = ChatEvent::ToolStatusUpdate {
                    tool_use_id: tool_use_id.clone(),
                    status: "awaiting_permission".to_string(),
                    permission_request_id: Some(permission_request_id.clone()),
                };
                ctx.event.emit(&awaiting_permission);

                // Wait for user response with timeout and abort support
                let allowed = crate::backends::utils::wait_for_permission(
                    ctx.permission_channels,
                    ctx.event.session_id,
                    &permission_request_id,
                    ctx.abort_notify,
                )
                .await;

                // If denied, remove from active_tools since tool won't run
                if !allowed {
                    active_tools.remove(&tool_use_id);
                }

                // Emit final status and send control response
                let (status, reason) = Self::pretool_prompt_resolution(allowed);
                let final_status = ChatEvent::ToolStatusUpdate {
                    tool_use_id: tool_use_id.clone(),
                    status: status.to_string(),
                    permission_request_id: None,
                };
                ctx.event.emit(&final_status);

                let response = if allowed {
                    OutgoingControlResponse::allow_pretool(request.request_id.clone(), reason)
                } else {
                    OutgoingControlResponse::deny_pretool(request.request_id.clone(), reason)
                };
                if let Err(e) = Self::send_control_response(ctx.stdin_writer, response).await {
                    log::error!("Failed to send permission response: {}", e);
                }
            }
        }
    }

    /// Handle result events (completion).
    async fn handle_result_event(
        event: ResultEvent,
        ctx: EventContext<'_>,
        in_flight: &Arc<AtomicBool>,
        current_turn_id: &Arc<RwLock<Option<String>>>,
    ) {
        if Self::is_error_result_subtype(&event.subtype) {
            let error_event = ChatEvent::Error {
                message: "CLI returned error".to_string(),
            };
            ctx.emit(&error_event);
        } else {
            let complete_event = ChatEvent::Complete;
            ctx.emit(&complete_event);
        }

        in_flight.store(false, Ordering::SeqCst);
        *current_turn_id.write().await = None;
        log::debug!("Turn completed: subtype={}", event.subtype);

        // Do not emit token usage from result totals here: result usage is
        // cumulative session accounting and does not match context usage semantics.
    }
}

#[cfg(test)]
mod tests {
    use super::{CliSession, HookCallbackAction};
    use crate::backends::claude::cli_protocol::{
        ControlRequestPayload, HookCallbackInput, IncomingControlRequest,
    };
    use serde_json::json;

    #[test]
    fn is_error_result_subtype_detects_error_like_values() {
        assert!(CliSession::is_error_result_subtype("error"));
        assert!(CliSession::is_error_result_subtype("failed"));
        assert!(CliSession::is_error_result_subtype("tool_error"));
        assert!(!CliSession::is_error_result_subtype("success"));
        assert!(!CliSession::is_error_result_subtype("cancelled"));
    }

    #[test]
    fn ack_hook_continue_uses_safe_continue_payload() {
        let response = CliSession::ack_hook_continue("req-1".to_string(), "Stop");
        let json_value = serde_json::to_value(response).expect("response should serialize");

        assert_eq!(json_value["type"], json!("control_response"));
        assert_eq!(json_value["response"]["subtype"], json!("success"));
        assert_eq!(json_value["response"]["request_id"], json!("req-1"));
        assert_eq!(json_value["response"]["response"]["continue"], json!(true));
        assert_eq!(
            json_value["response"]["response"]["hookSpecificOutput"]["hookEventName"],
            json!("Stop")
        );
        assert_eq!(
            json_value["response"]["response"]["hookSpecificOutput"]["permissionDecision"],
            json!("allow")
        );
        assert_eq!(
            json_value["response"]["response"]["hookSpecificOutput"]["permissionDecisionReason"],
            json!("Acknowledged")
        );
    }

    #[test]
    fn pretool_prompt_resolution_maps_user_decision() {
        assert_eq!(
            CliSession::pretool_prompt_resolution(true),
            ("running", "User approved")
        );
        assert_eq!(
            CliSession::pretool_prompt_resolution(false),
            ("denied", "User denied")
        );
    }

    #[test]
    fn classify_hook_callback_covers_hook_lifecycle_variants() {
        assert_eq!(
            CliSession::classify_hook_callback(&hook_request("hook_callback", Some("PreToolUse"))),
            HookCallbackAction::PreToolUse
        );
        assert_eq!(
            CliSession::classify_hook_callback(&hook_request("hook_callback", Some("PostToolUse"))),
            HookCallbackAction::PostToolUse
        );
        assert_eq!(
            CliSession::classify_hook_callback(&hook_request("hook_callback", Some("CustomHook"))),
            HookCallbackAction::UnknownHook("CustomHook".to_string())
        );
        assert_eq!(
            CliSession::classify_hook_callback(&hook_request("hook_callback", None)),
            HookCallbackAction::MissingInput
        );
        assert_eq!(
            CliSession::classify_hook_callback(&hook_request("initialize", Some("PreToolUse"))),
            HookCallbackAction::Ignore
        );
    }

    fn hook_request(subtype: &str, hook_event_name: Option<&str>) -> IncomingControlRequest {
        IncomingControlRequest {
            request_id: "req-test".to_string(),
            request: ControlRequestPayload {
                subtype: subtype.to_string(),
                callback_id: Some("cb-test".to_string()),
                tool_use_id: Some("tool-1".to_string()),
                input: hook_event_name.map(|name| HookCallbackInput {
                    hook_event_name: name.to_string(),
                    tool_name: None,
                    tool_input: None,
                }),
            },
        }
    }
}
