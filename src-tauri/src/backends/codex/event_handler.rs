//! Event handling for the Codex CLI backend.
//!
//! Contains the notification/request handlers and item processing logic
//! extracted from `adapter.rs`. All functions are associated functions on
//! `CodexSession` (no `&self`) so they can be called from the stdout reader
//! task that holds `Arc` clones of the required state.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, Notify, RwLock};
use uuid::Uuid;

use crate::backends::emit_chat_event;
use crate::backends::runtime::PermissionChannels;
use crate::backends::types::{ChatEvent, Message};

use super::adapter::CodexSession;
use super::cli_protocol::{
    clean_thinking_text, extract_approval_tool_info, final_tool_status, first_string,
    normalized_tool_from_item, token_usage_from_turn_completed, JsonRpcResponse,
};

#[derive(Debug, Clone, Copy)]
struct ThrottledLogState {
    last_logged_at: Instant,
    suppressed: u64,
}

static UNKNOWN_NOTIFICATION_LOGS: OnceLock<std::sync::Mutex<HashMap<String, ThrottledLogState>>> =
    OnceLock::new();

fn log_unknown_notification_throttled(method: &str) {
    const THROTTLE_WINDOW: Duration = Duration::from_secs(10);
    const MAX_TRACKED: usize = 256;

    let now = Instant::now();
    let mutex = UNKNOWN_NOTIFICATION_LOGS
        .get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let mut map = mutex.lock().unwrap_or_else(|err| err.into_inner());

    if map.len() > MAX_TRACKED {
        map.clear();
    }

    let entry = map.entry(method.to_string()).or_insert_with(|| ThrottledLogState {
        last_logged_at: now
            .checked_sub(THROTTLE_WINDOW)
            .unwrap_or(now),
        suppressed: 0,
    });

    if now.duration_since(entry.last_logged_at) >= THROTTLE_WINDOW {
        if entry.suppressed > 0 {
            log::debug!(
                "Ignoring unknown Codex notification: {} (suppressed {}x)",
                method,
                entry.suppressed
            );
        } else {
            log::debug!("Ignoring unknown Codex notification: {}", method);
        }
        entry.last_logged_at = now;
        entry.suppressed = 0;
    } else {
        entry.suppressed += 1;
    }
}

fn extract_notification_thread_id(params: &Value, item: &Value) -> Option<String> {
    first_string(
        params,
        &[
            &["senderThreadId"],
            &["sender_thread_id"],
            &["item", "senderThreadId"],
            &["item", "sender_thread_id"],
            &["threadId"],
            &["thread_id"],
            &["item", "threadId"],
            &["item", "thread_id"],
        ],
    )
    .or_else(|| {
        first_string(
            item,
            &[
                &["senderThreadId"],
                &["sender_thread_id"],
                &["threadId"],
                &["thread_id"],
            ],
        )
    })
    .map(|value| value.to_string())
}

fn extract_collab_wait_thread_id(params: &Value) -> Option<String> {
    first_string(
        params,
        &[
            &["id"],
            &["threadId"],
            &["thread_id"],
            &["item", "id"],
            &["item", "threadId"],
            &["item", "thread_id"],
            &["event", "id"],
            &["event", "threadId"],
            &["event", "thread_id"],
            &["payload", "id"],
            &["payload", "threadId"],
            &["payload", "thread_id"],
            &["data", "id"],
            &["data", "threadId"],
            &["data", "thread_id"],
        ],
    )
    .map(|value| value.to_string())
    .or_else(|| {
        [
            params.get("ids"),
            params.pointer("/item/ids"),
            params.pointer("/event/ids"),
            params.pointer("/payload/ids"),
            params.pointer("/data/ids"),
        ]
        .into_iter()
        .flatten()
        .find_map(|value| {
            value
                .as_array()
                .and_then(|ids| ids.first())
                .and_then(Value::as_str)
                .map(std::string::ToString::to_string)
        })
    })
}

fn first_receiver_thread_id(value: &Value) -> Option<String> {
    let ids = value
        .get("receiverThreadIds")
        .or_else(|| value.get("receiver_thread_ids"))
        .and_then(Value::as_array)?;
    ids.first()
        .and_then(Value::as_str)
        .map(std::string::ToString::to_string)
}

fn attach_thread_id_to_input(input: Option<Value>, thread_id: Option<String>) -> Option<Value> {
    let Some(thread_id) = thread_id else {
        return input;
    };

    match input {
        Some(Value::Object(mut map)) => {
            map.entry("__threadId".to_string())
                .or_insert_with(|| Value::String(thread_id));
            Some(Value::Object(map))
        }
        Some(other) => {
            let mut map = serde_json::Map::new();
            map.insert("arguments".to_string(), other);
            map.insert("__threadId".to_string(), Value::String(thread_id));
            Some(Value::Object(map))
        }
        None => {
            let mut map = serde_json::Map::new();
            map.insert("__threadId".to_string(), Value::String(thread_id));
            Some(Value::Object(map))
        }
    }
}

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

struct NotificationState<'a> {
    thread_id: &'a Arc<RwLock<Option<String>>>,
    codex_turn_id: &'a Arc<RwLock<Option<String>>>,
    in_flight: &'a AtomicBool,
    messages: &'a Arc<RwLock<Vec<Message>>>,
    active_collab_thread_id: &'a mut Option<String>,
    active_tools: &'a mut HashMap<String, String>,
    assistant_parts: &'a mut Vec<String>,
    reasoning_parts: &'a mut HashMap<String, String>,
}

#[derive(Clone, Copy)]
struct ApprovalContext<'a> {
    event: EventContext<'a>,
    permission_mode: &'a Arc<RwLock<String>>,
    permission_channels: &'a PermissionChannels,
    stdin_writer: &'a Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    abort_flag: &'a Arc<AtomicBool>,
    abort_notify: &'a Arc<Notify>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TurnTerminalOutcome {
    Completed,
    Failed,
    Cancelled,
    Aborted,
}

impl TurnTerminalOutcome {
    fn from_notification_method(method: &str) -> Option<Self> {
        match method {
            "turn/completed" => Some(Self::Completed),
            "turn/failed" => Some(Self::Failed),
            "turn/cancelled" | "turn/canceled" => Some(Self::Cancelled),
            "turn/aborted" => Some(Self::Aborted),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Aborted => "aborted",
        }
    }

    fn is_success(self) -> bool {
        matches!(self, Self::Completed)
    }
}

fn terminal_error_message(outcome: TurnTerminalOutcome, params: &Value) -> String {
    let detail = first_string(
        params,
        &[
            &["error", "message"],
            &["error"],
            &["message"],
            &["reason"],
            &["status"],
        ],
    )
    .unwrap_or("");

    if detail.is_empty() {
        format!("Codex turn {}", outcome.label())
    } else {
        format!("Codex turn {}: {detail}", outcome.label())
    }
}

fn approval_auto_accept(mode: &str, tool_type: &str) -> bool {
    matches!(
        (mode, tool_type),
        ("bypassPermissions", _) | ("acceptEdits", "file_change")
    )
}

fn approval_resolution(allowed: bool) -> (&'static str, &'static str) {
    if allowed {
        ("running", "accept")
    } else {
        ("denied", "decline")
    }
}

impl CodexSession {
    fn extract_item_from_notification<'a>(params: &'a Value) -> &'a Value {
        params
            .get("item")
            .or_else(|| params.pointer("/event/item"))
            .or_else(|| params.pointer("/payload/item"))
            .or_else(|| params.pointer("/data/item"))
            .unwrap_or(params)
    }

    fn extract_reasoning_delta(params: &Value) -> Option<(String, String)> {
        let item = Self::extract_item_from_notification(params);
        let item_id = first_string(item, &[&["id"]])
            .or_else(|| first_string(params, &[&["id"], &["itemId"], &["item_id"]]))
            .unwrap_or("")
            .to_string();

        if item_id.is_empty() {
            return None;
        }

        let delta = first_string(
            params,
            &[
                &["delta"],
                &["text"],
                &["content"],
                &["summaryText"],
                &["summary"],
                &["item", "delta"],
                &["item", "text"],
                &["item", "content"],
                &["item", "summaryText"],
                &["item", "summary"],
            ],
        )
        .unwrap_or("")
        .to_string();

        if delta.trim().is_empty() {
            return None;
        }

        Some((item_id, delta))
    }

    fn extract_approval_tool_use_id(params: &Value) -> Option<String> {
        first_string(
            params,
            &[
                &["toolUseId"],
                &["tool_use_id"],
                &["itemId"],
                &["item_id"],
                &["item", "id"],
                &["id"],
            ],
        )
        .map(|value| value.to_string())
    }

    async fn handle_turn_terminal_notification(
        outcome: TurnTerminalOutcome,
        params: &Value,
        event_ctx: EventContext<'_>,
        state: &mut NotificationState<'_>,
    ) {
        // Store assistant message if we accumulated delta chunks.
        let text = state.assistant_parts.join("");
        if !text.trim().is_empty() {
            let mut msgs = state.messages.write().await;
            msgs.push(Message::new("assistant", text));
        }
        state.assistant_parts.clear();
        state.active_tools.clear();
        state.reasoning_parts.clear();
        *state.active_collab_thread_id = None;

        // The turn is terminal regardless of outcome.
        *state.codex_turn_id.write().await = None;

        if let Some((total, ctx, window)) = token_usage_from_turn_completed(params) {
            let usage_event = ChatEvent::TokenUsage {
                total_tokens: total,
                context_tokens: ctx,
                context_window: window,
            };
            event_ctx.emit(&usage_event);
        }

        if outcome.is_success() {
            event_ctx.emit(&ChatEvent::Complete);
        } else {
            let error = ChatEvent::Error {
                message: terminal_error_message(outcome, params),
            };
            event_ctx.emit(&error);
        }

        state.in_flight.store(false, Ordering::SeqCst);
        log::debug!("Codex turn terminal notification: {}", outcome.label());
    }

    async fn handle_notification_with_context(
        method: &str,
        params: &Value,
        event_ctx: EventContext<'_>,
        state: &mut NotificationState<'_>,
    ) {
        if let Some(outcome) = TurnTerminalOutcome::from_notification_method(method) {
            Self::handle_turn_terminal_notification(outcome, params, event_ctx, state).await;
            return;
        }

        match method {
            "thread/started" => {
                if let Some(tid) = params
                    .pointer("/thread/id")
                    .or_else(|| params.get("threadId"))
                    .and_then(Value::as_str)
                {
                    *state.thread_id.write().await = Some(tid.to_string());
                }
            }

            "turn/started" => {
                if let Some(tid) = params
                    .pointer("/turn/id")
                    .or_else(|| params.get("turnId"))
                    .and_then(Value::as_str)
                {
                    *state.codex_turn_id.write().await = Some(tid.to_string());
                }
                // Clear accumulation for new turn
                state.assistant_parts.clear();
                state.active_tools.clear();
                state.reasoning_parts.clear();
                *state.active_collab_thread_id = None;
            }

            "item/started" | "codex/event/item_started" => {
                let default_thread_id = state.thread_id.read().await.clone();
                Self::handle_item_started(
                    params,
                    event_ctx,
                    state.active_tools,
                    state.active_collab_thread_id,
                    default_thread_id.as_deref(),
                );
            }

            "item/agentMessage/delta" | "item/delta" => {
                // Suppress text from child threads — only show parent/main thread text.
                let item = Self::extract_item_from_notification(params);
                let msg_thread_id = extract_notification_thread_id(params, item);
                let default_thread_id = state.thread_id.read().await.clone();
                let is_child_thread = match (&msg_thread_id, &default_thread_id) {
                    (Some(msg_tid), Some(main_tid)) => msg_tid != main_tid,
                    _ => false,
                };

                if !is_child_thread {
                    if let Some(text) = params
                        .get("delta")
                        .or_else(|| params.get("text"))
                        .and_then(Value::as_str)
                    {
                        if !text.is_empty() {
                            state.assistant_parts.push(text.to_string());
                            let event = ChatEvent::Text {
                                content: text.to_string(),
                            };
                            event_ctx.emit(&event);
                        }
                    }
                }
            }

            "item/completed" | "codex/event/item_completed" => {
                let default_thread_id = state.thread_id.read().await.clone();
                Self::handle_item_completed(
                    params,
                    event_ctx,
                    state.active_tools,
                    state.reasoning_parts,
                    state.active_collab_thread_id.as_deref(),
                    default_thread_id.as_deref(),
                );
            }

            "thread/tokenUsage/updated" => {
                if let Some((total, ctx, window)) = token_usage_from_turn_completed(params) {
                    let usage_event = ChatEvent::TokenUsage {
                        total_tokens: total,
                        context_tokens: ctx,
                        context_window: window,
                    };
                    event_ctx.emit(&usage_event);
                }
            }

            // Reasoning summary deltas (collect and emit once on item completion).
            "codex/event/reasoning_content_delta"
            | "codex/event/agent_reasoning_delta"
            | "item/reasoning/summaryTextDelta" => {
                if let Some((item_id, delta)) = Self::extract_reasoning_delta(params) {
                    let entry = state.reasoning_parts.entry(item_id).or_default();
                    entry.push_str(&delta);
                }
            }

            "codex/event/collab_waiting_begin" => {
                if let Some(collab_thread_id) = extract_collab_wait_thread_id(params) {
                    *state.active_collab_thread_id = Some(collab_thread_id);
                }
            }

            "codex/event/collab_agent_spawn_begin" => {
                if let Some(collab_thread_id) = extract_collab_wait_thread_id(params) {
                    *state.active_collab_thread_id = Some(collab_thread_id);
                }
            }

            "codex/event/collab_waiting_end" => {
                *state.active_collab_thread_id = None;
            }

            // High-frequency notifications we intentionally ignore (avoid log spam).
            "codex/event/agent_reasoning"
            | "codex/event/web_search_begin"
            | "codex/event/web_search_end"
            | "thread/status/changed"
            | "codex/event/mcp_startup_complete"
            | "codex/event/task_started"
            | "codex/event/task_complete"
            | "codex/event/user_message"
            | "codex/event/stream_error"
            | "codex/event/warning"
            | "codex/event/token_count"
            | "account/rateLimits/updated"
            | "codex/event/collab_agent_spawn_end"
            | "codex/event/agent_reasoning_section_break"
            | "item/reasoning/summaryPartAdded"
            | "codex/event/agent_message"
            | "codex/event/agent_message_delta"
            | "codex/event/agent_message_content_delta"
            | "codex/event/exec_command_begin"
            | "codex/event/exec_command_output_delta"
            | "item/commandExecution/outputDelta"
            | "codex/event/exec_command_end"
            | "error" => {}

            _ => {
                log_unknown_notification_throttled(method);
            }
        }
    }

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
        active_collab_thread_id: &mut Option<String>,
        assistant_parts: &mut Vec<String>,
        reasoning_parts: &mut HashMap<String, String>,
    ) {
        let event_ctx = EventContext {
            app_handle,
            session_id,
            turn_id,
        };
        let mut state = NotificationState {
            thread_id,
            codex_turn_id,
            in_flight,
            messages,
            active_collab_thread_id,
            active_tools,
            assistant_parts,
            reasoning_parts,
        };

        Self::handle_notification_with_context(method, params, event_ctx, &mut state).await;
    }

    fn handle_item_started(
        params: &Value,
        event_ctx: EventContext<'_>,
        active_tools: &mut HashMap<String, String>,
        active_collab_thread_id: &mut Option<String>,
        default_thread_id: Option<&str>,
    ) {
        let item = Self::extract_item_from_notification(params);
        let item_kind = first_string(item, &[&["type"], &["kind"]])
            .or_else(|| first_string(params, &[&["item_type"], &["kind"]]))
            .unwrap_or("tool");
        let item_id = first_string(item, &[&["id"]])
            .or_else(|| first_string(params, &[&["item_id"], &["id"]]))
            .unwrap_or("item")
            .to_string();

        let kind_lower = item_kind.to_lowercase();

        if kind_lower.contains("reason") {
            // Reasoning summaries arrive as delta notifications; we emit a single ThinkingStart
            // once the item completes (if we have non-empty content).
        } else if kind_lower.contains("message") {
            // User/agent messages handled via delta/completed -- skip
        } else {
            // Some Codex versions emit both `item/started` and `codex/event/item_started`
            // for the same item id. Only emit ToolStart once per id.
            if active_tools.contains_key(&item_id) {
                return;
            }
            let (tool_type, target, input) = normalized_tool_from_item(item);
            if tool_type == "spawn_agent" {
                if let Some(thread_id) = input.as_ref().and_then(first_receiver_thread_id) {
                    *active_collab_thread_id = Some(thread_id);
                }
            }
            if tool_type == "wait" && !target.trim().is_empty() {
                *active_collab_thread_id = Some(target.clone());
            } else if tool_type == "wait" {
                let wait_thread_id = input
                    .as_ref()
                    .and_then(|value| value.get("ids"))
                    .and_then(Value::as_array)
                    .and_then(|ids| ids.first())
                    .and_then(Value::as_str)
                    .map(std::string::ToString::to_string);
                if let Some(thread_id) = wait_thread_id {
                    *active_collab_thread_id = Some(thread_id);
                }
            }
            // Avoid showing placeholder/empty "bash" tools while Codex is still preparing the command.
            // We'll update/emit the real target from the approval request when available.
            if tool_type == "command_execution" && target.trim().is_empty() {
                active_tools.insert(item_id.clone(), tool_type);
                return;
            }
            let collab_thread_id = active_collab_thread_id.clone();
            let input = attach_thread_id_to_input(
                input,
                extract_notification_thread_id(params, item)
                    .or(collab_thread_id)
                    .or_else(|| default_thread_id.map(std::string::ToString::to_string)),
            );
            active_tools.insert(item_id.clone(), tool_type.clone());
            let event = ChatEvent::ToolStart {
                tool_use_id: item_id.clone(),
                tool_type,
                target,
                status: "running".to_string(),
                input,
            };
            event_ctx.emit(&event);
        }
    }

    fn handle_item_completed(
        params: &Value,
        event_ctx: EventContext<'_>,
        active_tools: &mut HashMap<String, String>,
        reasoning_parts: &mut HashMap<String, String>,
        active_collab_thread_id: Option<&str>,
        default_thread_id: Option<&str>,
    ) {
        let item = Self::extract_item_from_notification(params);
        let item_kind = first_string(item, &[&["type"], &["kind"]])
            .or_else(|| first_string(params, &[&["item_type"], &["kind"]]))
            .unwrap_or("tool");
        let item_id = first_string(item, &[&["id"]])
            .or_else(|| first_string(params, &[&["item_id"], &["id"]]))
            .unwrap_or("item")
            .to_string();

        let kind_lower = item_kind.to_lowercase();

        if kind_lower.contains("reason") {
            let raw = reasoning_parts.remove(&item_id).unwrap_or_else(|| {
                first_string(item, &[&["text"], &["summaryText"], &["summary"]])
                    .unwrap_or("")
                    .to_string()
            });
            let thinking_content = clean_thinking_text(raw.as_str());
            if !thinking_content.trim().is_empty() && thinking_content.trim() != "Thinking" {
                let thread_id = extract_notification_thread_id(params, item)
                    .or_else(|| active_collab_thread_id.map(std::string::ToString::to_string))
                    .or_else(|| default_thread_id.map(std::string::ToString::to_string));
                let input = attach_thread_id_to_input(None, thread_id);
                let start = ChatEvent::ThinkingStart {
                    thinking_id: item_id.clone(),
                    content: thinking_content,
                    input,
                };
                event_ctx.emit(&start);
            }
        } else if kind_lower.contains("message") {
            // Text already emitted via item/agentMessage/delta — nothing to do here
        } else if (item_kind == "webSearch" || item_kind == "web_search_call")
            && !active_tools.contains_key(&item_id)
        {
            let target = first_string(item, &[&["action", "query"], &["query"]])
                .unwrap_or("")
                .to_string();
            let input = attach_thread_id_to_input(
                None,
                extract_notification_thread_id(params, item)
                    .or_else(|| active_collab_thread_id.map(std::string::ToString::to_string))
                    .or_else(|| default_thread_id.map(std::string::ToString::to_string)),
            );
            let start = ChatEvent::ToolStart {
                tool_use_id: item_id.clone(),
                tool_type: "web_search".to_string(),
                target,
                status: "pending".to_string(),
                input,
            };
            event_ctx.emit(&start);
            let end = ChatEvent::ToolEnd {
                id: item_id,
                status: "completed".to_string(),
                output: None,
            };
            event_ctx.emit(&end);
        } else if (item_kind == "fileChange" || item_kind == "file_change")
            && !active_tools.contains_key(&item_id)
        {
            let target = item
                .get("changes")
                .and_then(Value::as_array)
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("path"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let input = attach_thread_id_to_input(
                None,
                extract_notification_thread_id(params, item)
                    .or_else(|| active_collab_thread_id.map(std::string::ToString::to_string))
                    .or_else(|| default_thread_id.map(std::string::ToString::to_string)),
            );
            let start = ChatEvent::ToolStart {
                tool_use_id: item_id.clone(),
                tool_type: "file_change".to_string(),
                target,
                status: "pending".to_string(),
                input,
            };
            event_ctx.emit(&start);
            let end = ChatEvent::ToolEnd {
                id: item_id,
                status: "completed".to_string(),
                output: None,
            };
            event_ctx.emit(&end);
        } else if active_tools.contains_key(&item_id) {
            let tool_type = active_tools
                .remove(&item_id)
                .unwrap_or_else(|| item_kind.to_string());
            let completed_status = first_string(item, &[&["status"]]).unwrap_or("completed");
            let exit_code = item
                .get("exitCode")
                .or_else(|| item.get("exit_code"))
                .and_then(Value::as_i64);
            let status = final_tool_status(&tool_type, completed_status, exit_code);
            let end = ChatEvent::ToolEnd {
                id: item_id,
                status: status.to_string(),
                output: item.get("output").cloned(),
            };
            event_ctx.emit(&end);
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
        active_collab_thread_id: &mut Option<String>,
        assistant_parts: &mut Vec<String>,
        reasoning_parts: &mut HashMap<String, String>,
    ) {
        let event_ctx = EventContext {
            app_handle,
            session_id,
            turn_id,
        };

        // Map legacy event types to notification methods
        let (method, params) = match kind {
            "thread.started" => ("thread/started", parsed.clone()),
            "turn.started" => ("turn/started", parsed.clone()),
            "item.started" => ("item/started", parsed.clone()),
            "item.completed" => ("item/completed", parsed.clone()),
            "turn.completed" => ("turn/completed", parsed.clone()),
            "turn.failed" => ("turn/failed", parsed.clone()),
            "turn.cancelled" | "turn.canceled" => ("turn/cancelled", parsed.clone()),
            "turn.aborted" => ("turn/aborted", parsed.clone()),
            "error" => {
                if let Some(err) =
                    first_string(parsed, &[&["message"], &["error"], &["error", "message"]])
                {
                    let event = ChatEvent::Error {
                        message: err.to_string(),
                    };
                    event_ctx.emit(&event);
                }
                return;
            }
            _ => {
                // Try to extract text from unknown events
                let item_kind =
                    first_string(parsed, &[&["item", "type"], &["item_type"], &["kind"]])
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
                            event_ctx.emit(&event);
                        }
                    }
                }
                return;
            }
        };

        let mut state = NotificationState {
            thread_id,
            codex_turn_id,
            in_flight,
            messages,
            active_collab_thread_id,
            active_tools,
            assistant_parts,
            reasoning_parts,
        };

        Self::handle_notification_with_context(method, &params, event_ctx, &mut state).await;
    }

    async fn handle_approval_request_with_context(
        request_id: Value,
        method: &str,
        params: &Value,
        ctx: ApprovalContext<'_>,
        active_tools: &mut HashMap<String, String>,
    ) {
        let (tool_type, target) = extract_approval_tool_info(method, params);
        let tool_use_id =
            Self::extract_approval_tool_use_id(params).unwrap_or_else(|| Uuid::new_v4().to_string());

        // Emit ToolStart with pending status
        active_tools.insert(tool_use_id.clone(), tool_type.clone());
        let start_event = ChatEvent::ToolStart {
            tool_use_id: tool_use_id.clone(),
            tool_type: tool_type.clone(),
            target: target.clone(),
            status: "pending".to_string(),
            input: None,
        };
        ctx.event.emit(&start_event);

        let mode = ctx.permission_mode.read().await.clone();
        log::debug!(
            "Approval request: tool={}, target={}, mode={}",
            tool_type,
            target,
            mode
        );

        // Decide whether to auto-accept or prompt user
        let auto_accept = approval_auto_accept(mode.as_str(), &tool_type);

        let allowed = if auto_accept {
            // Auto-accept: update status to running
            let running_event = ChatEvent::ToolStatusUpdate {
                tool_use_id: tool_use_id.clone(),
                status: "running".to_string(),
                permission_request_id: None,
            };
            ctx.event.emit(&running_event);
            true
        } else {
            // Prompt user: emit awaiting_permission and wait
            let permission_request_id = Uuid::new_v4().to_string();

            let awaiting_event = ChatEvent::ToolStatusUpdate {
                tool_use_id: tool_use_id.clone(),
                status: "awaiting_permission".to_string(),
                permission_request_id: Some(permission_request_id.clone()),
            };
            ctx.event.emit(&awaiting_event);

            // Wait for user decision, timeout, or abort
            let decision = crate::backends::utils::wait_for_permission(
                ctx.permission_channels,
                ctx.event.session_id,
                &permission_request_id,
                ctx.abort_notify,
            )
            .await;

            // Emit status update
            let (status, _) = approval_resolution(decision);
            let status_event = ChatEvent::ToolStatusUpdate {
                tool_use_id: tool_use_id.clone(),
                status: status.to_string(),
                permission_request_id: None,
            };
            ctx.event.emit(&status_event);

            decision
        };

        // Send the approval response to the server
        let (_, decision_str) = approval_resolution(allowed);
        let response = JsonRpcResponse::new(request_id, json!({ "decision": decision_str }));

        let mut line = match serde_json::to_string(&response) {
            Ok(l) => l,
            Err(_) => return,
        };
        line.push('\n');

        let mut guard = ctx.stdin_writer.lock().await;
        if let Some(writer) = guard.as_mut() {
            let _ = writer.write_all(line.as_bytes()).await;
            let _ = writer.flush().await;
        }

        // If denied and abort was requested, remove from active tools
        if !allowed && ctx.abort_flag.load(Ordering::SeqCst) {
            active_tools.remove(&tool_use_id);
        }
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
        let ctx = ApprovalContext {
            event: EventContext {
                app_handle,
                session_id,
                turn_id,
            },
            permission_mode,
            permission_channels,
            stdin_writer,
            abort_flag,
            abort_notify,
        };

        Self::handle_approval_request_with_context(request_id, method, params, ctx, active_tools)
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        approval_auto_accept, approval_resolution, attach_thread_id_to_input,
        extract_notification_thread_id, terminal_error_message, TurnTerminalOutcome,
    };
    use crate::backends::codex::cli_protocol::{normalized_tool_from_item, IncomingMessage};
    use serde_json::json;
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;

    fn load_fixture_notifications(name: &str) -> Vec<(String, Value)> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("codex")
            .join(name);
        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("Failed to read fixture {:?}: {}", path, err));

        raw.lines()
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| {
                let value: Value = serde_json::from_str(line).unwrap_or_else(|err| {
                    panic!("Invalid JSON line in fixture {:?}: {}", path, err)
                });
                match IncomingMessage::parse(&value) {
                    Some(IncomingMessage::Notification { method, params }) => Some((method, params)),
                    _ => None,
                }
            })
            .collect()
    }

    #[test]
    fn extract_thread_id_prefers_sender_thread_id_over_thread_id() {
        let params = json!({
            "threadId": "thread-main",
            "item": { "senderThreadId": "thread-parent" }
        });
        let item = params.get("item").cloned().unwrap_or_else(|| json!({}));

        let thread_id = extract_notification_thread_id(&params, &item);
        assert_eq!(thread_id.as_deref(), Some("thread-parent"));
    }

    #[test]
    fn extract_thread_id_falls_back_to_sender_thread_id() {
        let params = json!({
            "item": { "senderThreadId": "thread-parent" }
        });
        let item = params.get("item").cloned().unwrap_or_else(|| json!({}));

        let thread_id = extract_notification_thread_id(&params, &item);
        assert_eq!(thread_id.as_deref(), Some("thread-parent"));
    }

    #[test]
    fn extract_thread_id_prefers_item_sender_thread_id_over_thread_id() {
        let params = json!({
            "threadId": "thread-parent",
            "item": { "senderThreadId": "thread-child" }
        });
        let item = params.get("item").cloned().unwrap_or_else(|| json!({}));

        let thread_id = extract_notification_thread_id(&params, &item);
        assert_eq!(thread_id.as_deref(), Some("thread-child"));
    }

    #[test]
    fn attach_thread_id_adds_metadata_without_clobbering_input() {
        let input = json!({
            "receiverThreadIds": ["thread-child-1"]
        });

        let enriched = attach_thread_id_to_input(Some(input), Some("thread-parent".to_string()))
            .expect("thread metadata should be attached");

        assert_eq!(enriched.get("__threadId"), Some(&json!("thread-parent")));
        assert_eq!(
            enriched.get("receiverThreadIds"),
            Some(&json!(["thread-child-1"]))
        );
    }

    #[test]
    fn terminal_notification_classifies_expected_variants() {
        assert_eq!(
            TurnTerminalOutcome::from_notification_method("turn/completed"),
            Some(TurnTerminalOutcome::Completed)
        );
        assert_eq!(
            TurnTerminalOutcome::from_notification_method("turn/failed"),
            Some(TurnTerminalOutcome::Failed)
        );
        assert_eq!(
            TurnTerminalOutcome::from_notification_method("turn/cancelled"),
            Some(TurnTerminalOutcome::Cancelled)
        );
        assert_eq!(
            TurnTerminalOutcome::from_notification_method("turn/canceled"),
            Some(TurnTerminalOutcome::Cancelled)
        );
        assert_eq!(
            TurnTerminalOutcome::from_notification_method("turn/aborted"),
            Some(TurnTerminalOutcome::Aborted)
        );
        assert_eq!(
            TurnTerminalOutcome::from_notification_method("turn/started"),
            None
        );
    }

    #[test]
    fn terminal_error_message_prefers_embedded_error_detail() {
        let params = json!({
            "error": { "message": "policy violation" }
        });
        let message = terminal_error_message(TurnTerminalOutcome::Failed, &params);
        assert_eq!(message, "Codex turn failed: policy violation");
    }

    #[test]
    fn terminal_outcome_labels_and_success_flags_match_semantics() {
        assert_eq!(TurnTerminalOutcome::Completed.label(), "completed");
        assert_eq!(TurnTerminalOutcome::Failed.label(), "failed");
        assert_eq!(TurnTerminalOutcome::Cancelled.label(), "cancelled");
        assert_eq!(TurnTerminalOutcome::Aborted.label(), "aborted");

        assert!(TurnTerminalOutcome::Completed.is_success());
        assert!(!TurnTerminalOutcome::Failed.is_success());
        assert!(!TurnTerminalOutcome::Cancelled.is_success());
        assert!(!TurnTerminalOutcome::Aborted.is_success());
    }

    #[test]
    fn terminal_error_message_falls_back_to_reason_then_generic() {
        let with_reason = json!({
            "reason": "interrupted by policy"
        });
        assert_eq!(
            terminal_error_message(TurnTerminalOutcome::Cancelled, &with_reason),
            "Codex turn cancelled: interrupted by policy"
        );

        let generic = json!({});
        assert_eq!(
            terminal_error_message(TurnTerminalOutcome::Aborted, &generic),
            "Codex turn aborted"
        );
    }

    #[test]
    fn approval_auto_accept_covers_permission_modes() {
        assert!(approval_auto_accept("bypassPermissions", "shell"));
        assert!(approval_auto_accept("acceptEdits", "file_change"));
        assert!(!approval_auto_accept("acceptEdits", "shell"));
        assert!(!approval_auto_accept("default", "file_change"));
    }

    #[test]
    fn approval_resolution_maps_prompt_decision_to_status_and_response() {
        assert_eq!(approval_resolution(true), ("running", "accept"));
        assert_eq!(approval_resolution(false), ("denied", "decline"));
    }

    #[test]
    fn fixture_subagent_contract_preserves_spawn_and_child_thread_signals() {
        let notifications = load_fixture_notifications("subagent_contract.jsonl");
        assert_eq!(notifications.len(), 2);

        let (spawn_method, spawn_params) = &notifications[0];
        assert_eq!(spawn_method, "item/started");
        let spawn_item = super::CodexSession::extract_item_from_notification(spawn_params);
        let (tool_type, target, input) = normalized_tool_from_item(spawn_item);
        assert_eq!(tool_type, "spawn_agent");
        assert_eq!(target, "Research release notes");
        assert_eq!(
            input
                .as_ref()
                .and_then(|value| value.get("receiver_thread_ids"))
                .and_then(Value::as_array)
                .and_then(|values| values.first())
                .and_then(Value::as_str),
            Some("thread-child-a")
        );

        let (child_method, child_params) = &notifications[1];
        assert_eq!(child_method, "item/started");
        let child_item = super::CodexSession::extract_item_from_notification(child_params);
        let thread_id = extract_notification_thread_id(child_params, child_item);
        assert_eq!(thread_id.as_deref(), Some("thread-child-a"));
    }
}
