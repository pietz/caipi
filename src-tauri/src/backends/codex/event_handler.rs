//! Event handling for the Codex CLI backend.
//!
//! Contains the notification/request handlers and item processing logic
//! extracted from `adapter.rs`. All functions are associated functions on
//! `CodexSession` (no `&self`) so they can be called from the stdout reader
//! task that holds `Arc` clones of the required state.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use serde_json::{json, Value};
use tauri::AppHandle;
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, Notify, RwLock};
use uuid::Uuid;

use crate::backends::emit_chat_event;
use crate::backends::runtime::PermissionChannels;
use crate::commands::chat::{ChatEvent, Message};

use super::adapter::CodexSession;
use super::cli_protocol::{
    clean_thinking_text, extract_approval_tool_info, first_string,
    normalized_tool_from_item, token_usage_from_turn_completed, final_tool_status,
    JsonRpcResponse,
};

impl CodexSession {
    /// Handle a JSON-RPC notification from the app-server.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn handle_notification(
        method: &str,
        params: &Value,
        app_handle: &AppHandle,
        session_id: &str,
        turn_id: Option<&str>,
        thread_id: &Arc<RwLock<Option<String>>>,
        codex_turn_id: &Arc<RwLock<Option<String>>>,
        in_flight: &AtomicBool,
        messages: &Arc<RwLock<Vec<Message>>>,
        active_tools: &mut HashMap<String, String>,
        assistant_parts: &mut Vec<String>,
    ) {
        match method {
            "thread/started" => {
                if let Some(tid) = params
                    .pointer("/thread/id")
                    .or_else(|| params.get("threadId"))
                    .and_then(Value::as_str)
                {
                    *thread_id.write().await = Some(tid.to_string());
                }
            }

            "turn/started" => {
                if let Some(tid) = params
                    .pointer("/turn/id")
                    .or_else(|| params.get("turnId"))
                    .and_then(Value::as_str)
                {
                    *codex_turn_id.write().await = Some(tid.to_string());
                }
                // Clear accumulation for new turn
                assistant_parts.clear();
                active_tools.clear();
            }

            "item/started" => {
                Self::handle_item_started(params, app_handle, session_id, turn_id, active_tools);
            }

            "item/agentMessage/delta" | "item/delta" => {
                if let Some(text) = params
                    .get("delta")
                    .or_else(|| params.get("text"))
                    .and_then(Value::as_str)
                {
                    if !text.is_empty() {
                        assistant_parts.push(text.to_string());
                        let event = ChatEvent::Text {
                            content: text.to_string(),
                        };
                        emit_chat_event(app_handle, Some(session_id), turn_id, &event);
                    }
                }
            }

            "item/completed" => {
                Self::handle_item_completed(
                    params,
                    app_handle,
                    session_id,
                    turn_id,
                    active_tools,
                    assistant_parts,
                );
            }

            "thread/tokenUsage/updated" => {
                if let Some((total, ctx, window)) = token_usage_from_turn_completed(params) {
                    let usage_event = ChatEvent::TokenUsage {
                        total_tokens: total,
                        context_tokens: ctx,
                        context_window: window,
                    };
                    emit_chat_event(app_handle, Some(session_id), turn_id, &usage_event);
                }
            }

            "turn/completed" => {
                // Store assistant message
                let text = assistant_parts.join("");
                if !text.trim().is_empty() {
                    let mut msgs = messages.write().await;
                    msgs.push(Message::new("assistant", text));
                }
                assistant_parts.clear();

                // Clear codex turn id
                *codex_turn_id.write().await = None;

                // Fallback: extract usage from turn/completed params (legacy)
                if let Some((total, ctx, window)) = token_usage_from_turn_completed(params) {
                    let usage_event = ChatEvent::TokenUsage {
                        total_tokens: total,
                        context_tokens: ctx,
                        context_window: window,
                    };
                    emit_chat_event(app_handle, Some(session_id), turn_id, &usage_event);
                }

                // Emit completion
                log::debug!("Codex turn completed, message stored");
                let complete_event = ChatEvent::Complete;
                emit_chat_event(app_handle, Some(session_id), turn_id, &complete_event);
                in_flight.store(false, Ordering::SeqCst);
            }

            _ => {
                // Unknown notification -- ignore
            }
        }
    }

    pub(super) fn handle_item_started(
        params: &Value,
        app_handle: &AppHandle,
        session_id: &str,
        turn_id: Option<&str>,
        active_tools: &mut HashMap<String, String>,
    ) {
        let item = params.get("item").unwrap_or(params);
        let item_kind = first_string(item, &[&["type"], &["kind"]])
            .or_else(|| first_string(params, &[&["item_type"], &["kind"]]))
            .unwrap_or("tool");
        let item_id = first_string(item, &[&["id"]])
            .or_else(|| first_string(params, &[&["item_id"], &["id"]]))
            .unwrap_or("item")
            .to_string();

        let kind_lower = item_kind.to_lowercase();

        if kind_lower.contains("reason") {
            let thinking_content =
                clean_thinking_text(first_string(item, &[&["text"]]).unwrap_or("Thinking"));
            let event = ChatEvent::ThinkingStart {
                thinking_id: item_id,
                content: thinking_content,
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &event);
        } else if kind_lower.contains("message") {
            // User/agent messages handled via delta/completed -- skip
        } else {
            let (tool_type, target, input) = normalized_tool_from_item(item);
            active_tools.insert(item_id.clone(), tool_type.clone());
            let event = ChatEvent::ToolStart {
                tool_use_id: item_id.clone(),
                tool_type,
                target,
                status: "running".to_string(),
                input,
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &event);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn handle_item_completed(
        params: &Value,
        app_handle: &AppHandle,
        session_id: &str,
        turn_id: Option<&str>,
        active_tools: &mut HashMap<String, String>,
        _assistant_parts: &mut Vec<String>,
    ) {
        let item = params.get("item").unwrap_or(params);
        let item_kind = first_string(item, &[&["type"], &["kind"]])
            .or_else(|| first_string(params, &[&["item_type"], &["kind"]]))
            .unwrap_or("tool");
        let item_id = first_string(item, &[&["id"]])
            .or_else(|| first_string(params, &[&["item_id"], &["id"]]))
            .unwrap_or("item")
            .to_string();

        let kind_lower = item_kind.to_lowercase();

        if kind_lower.contains("reason") {
            let thinking_content =
                clean_thinking_text(first_string(item, &[&["text"]]).unwrap_or("Thinking"));
            // Emit start+end for reasoning blocks
            let start = ChatEvent::ThinkingStart {
                thinking_id: item_id.clone(),
                content: thinking_content,
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &start);
            let end = ChatEvent::ThinkingEnd {
                thinking_id: item_id,
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &end);
        } else if kind_lower.contains("message") {
            // Text already emitted via item/agentMessage/delta — nothing to do here
        } else if (item_kind == "webSearch" || item_kind == "web_search_call") && !active_tools.contains_key(&item_id) {
            let target = first_string(item, &[&["action", "query"], &["query"]])
                .unwrap_or("")
                .to_string();
            let start = ChatEvent::ToolStart {
                tool_use_id: item_id.clone(),
                tool_type: "web_search".to_string(),
                target,
                status: "pending".to_string(),
                input: None,
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &start);
            let end = ChatEvent::ToolEnd {
                id: item_id,
                status: "completed".to_string(),
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &end);
        } else if (item_kind == "fileChange" || item_kind == "file_change") && !active_tools.contains_key(&item_id) {
            let target = item
                .get("changes")
                .and_then(Value::as_array)
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("path"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let start = ChatEvent::ToolStart {
                tool_use_id: item_id.clone(),
                tool_type: "file_change".to_string(),
                target,
                status: "pending".to_string(),
                input: None,
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &start);
            let end = ChatEvent::ToolEnd {
                id: item_id,
                status: "completed".to_string(),
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &end);
        } else if active_tools.contains_key(&item_id) {
            let tool_type = active_tools
                .remove(&item_id)
                .unwrap_or_else(|| item_kind.to_string());
            let completed_status = first_string(item, &[&["status"]]).unwrap_or("completed");
            let exit_code = item.get("exitCode").or_else(|| item.get("exit_code")).and_then(Value::as_i64);
            let status = final_tool_status(&tool_type, completed_status, exit_code);
            let end = ChatEvent::ToolEnd {
                id: item_id,
                status: status.to_string(),
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &end);
        }
    }

    /// Handle legacy-format events (type-based rather than JSON-RPC method-based).
    /// This provides backwards compatibility with older Codex versions.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn handle_legacy_event(
        kind: &str,
        parsed: &Value,
        app_handle: &AppHandle,
        session_id: &str,
        turn_id: Option<&str>,
        thread_id: &Arc<RwLock<Option<String>>>,
        codex_turn_id: &Arc<RwLock<Option<String>>>,
        in_flight: &AtomicBool,
        messages: &Arc<RwLock<Vec<Message>>>,
        active_tools: &mut HashMap<String, String>,
        assistant_parts: &mut Vec<String>,
    ) {
        // Map legacy event types to notification methods
        let (method, params) = match kind {
            "thread.started" => ("thread/started", parsed.clone()),
            "turn.started" => ("turn/started", parsed.clone()),
            "item.started" => ("item/started", parsed.clone()),
            "item.completed" => ("item/completed", parsed.clone()),
            "turn.completed" => ("turn/completed", parsed.clone()),
            "error" => {
                if let Some(err) =
                    first_string(parsed, &[&["message"], &["error"], &["error", "message"]])
                {
                    let event = ChatEvent::Error {
                        message: err.to_string(),
                    };
                    emit_chat_event(app_handle, Some(session_id), turn_id, &event);
                }
                return;
            }
            _ => {
                // Try to extract text from unknown events
                let item_kind = first_string(parsed, &[&["item", "type"], &["item_type"], &["kind"]])
                    .unwrap_or("");
                let should_emit_text = !(item_kind.contains("reason")
                    || item_kind == "command_execution"
                    || item_kind == "function_call"
                    || item_kind == "web_search"
                    || item_kind == "web_search_call"
                    || item_kind == "file_change");

                if should_emit_text {
                    if let Some(text) = first_string(
                        parsed,
                        &[
                            &["delta"],
                            &["text"],
                            &["content"],
                            &["item", "text"],
                            &["item", "content", "text"],
                            &["message", "content", "text"],
                        ],
                    ) {
                        if !text.is_empty() {
                            assistant_parts.push(text.to_string());
                            let event = ChatEvent::Text {
                                content: text.to_string(),
                            };
                            emit_chat_event(app_handle, Some(session_id), turn_id, &event);
                        }
                    }
                }
                return;
            }
        };

        Self::handle_notification(
            method,
            &params,
            app_handle,
            session_id,
            turn_id,
            thread_id,
            codex_turn_id,
            in_flight,
            messages,
            active_tools,
            assistant_parts,
        )
        .await;
    }

    /// Handle an incoming approval request from the server.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn handle_approval_request(
        request_id: Value,
        method: &str,
        params: &Value,
        app_handle: &AppHandle,
        session_id: &str,
        turn_id: Option<&str>,
        permission_mode: &Arc<RwLock<String>>,
        permission_channels: &PermissionChannels,
        stdin_writer: &Arc<Mutex<Option<tokio::process::ChildStdin>>>,
        abort_flag: &Arc<AtomicBool>,
        abort_notify: &Arc<Notify>,
        active_tools: &mut HashMap<String, String>,
    ) {
        let (tool_type, target) = extract_approval_tool_info(method, params);
        let tool_use_id = Uuid::new_v4().to_string();

        // Emit ToolStart with pending status
        active_tools.insert(tool_use_id.clone(), tool_type.clone());
        let start_event = ChatEvent::ToolStart {
            tool_use_id: tool_use_id.clone(),
            tool_type: tool_type.clone(),
            target: target.clone(),
            status: "pending".to_string(),
            input: None,
        };
        emit_chat_event(app_handle, Some(session_id), turn_id, &start_event);

        let mode = permission_mode.read().await.clone();
        log::debug!("Approval request: tool={}, target={}, mode={}", tool_type, target, mode);

        // Decide whether to auto-accept or prompt user
        let auto_accept = match mode.as_str() {
            "bypassPermissions" => true,
            "acceptEdits" if tool_type == "file_change" => true,
            _ => false,
        };

        let allowed = if auto_accept {
            // Auto-accept: update status to running
            let running_event = ChatEvent::ToolStatusUpdate {
                tool_use_id: tool_use_id.clone(),
                status: "running".to_string(),
                permission_request_id: None,
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &running_event);
            true
        } else {
            // Prompt user: emit awaiting_permission and wait
            let permission_request_id = Uuid::new_v4().to_string();

            let awaiting_event = ChatEvent::ToolStatusUpdate {
                tool_use_id: tool_use_id.clone(),
                status: "awaiting_permission".to_string(),
                permission_request_id: Some(permission_request_id.clone()),
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &awaiting_event);

            // Wait for user decision, timeout, or abort
            let decision = crate::backends::utils::wait_for_permission(
                permission_channels,
                &permission_request_id,
                abort_notify,
            )
            .await;

            // Emit status update
            let status = if decision { "running" } else { "denied" };
            let status_event = ChatEvent::ToolStatusUpdate {
                tool_use_id: tool_use_id.clone(),
                status: status.to_string(),
                permission_request_id: None,
            };
            emit_chat_event(app_handle, Some(session_id), turn_id, &status_event);

            decision
        };

        // Send the approval response to the server
        let decision_str = if allowed { "accept" } else { "decline" };
        let response = JsonRpcResponse::new(
            request_id,
            json!({ "decision": decision_str }),
        );

        let mut line = match serde_json::to_string(&response) {
            Ok(l) => l,
            Err(_) => return,
        };
        line.push('\n');

        let mut guard = stdin_writer.lock().await;
        if let Some(writer) = guard.as_mut() {
            let _ = writer.write_all(line.as_bytes()).await;
            let _ = writer.flush().await;
        }

        // If denied and abort was requested, remove from active tools
        if !allowed && abort_flag.load(Ordering::SeqCst) {
            active_tools.remove(&tool_use_id);
        }
    }
}
