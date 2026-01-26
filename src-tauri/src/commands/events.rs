//! Event emission helpers for Claude chat events.
//!
//! Centralizes all event emission to ensure consistent formatting
//! and simplify the emission of various event types.
//!
//! Note: These are currently unused as events are emitted directly from hooks,
//! but kept for potential future use.

#![allow(dead_code)]

use tauri::{AppHandle, Emitter};

use super::chat::ChatEvent;

const EVENT_NAME: &str = "claude:event";

/// Emit a text event with streaming content
pub fn emit_text(app: &AppHandle, content: String) {
    let _ = app.emit(EVENT_NAME, &ChatEvent::Text { content });
}

/// Emit a tool start event (emitted from PreToolUse hook)
pub fn emit_tool_start(
    app: &AppHandle,
    tool_use_id: String,
    tool_type: String,
    target: String,
    status: String,
    input: Option<serde_json::Value>,
) {
    let _ = app.emit(EVENT_NAME, &ChatEvent::ToolStart {
        tool_use_id,
        tool_type,
        target,
        status,
        input,
    });
}

/// Emit a tool status update event
pub fn emit_tool_status_update(
    app: &AppHandle,
    tool_use_id: String,
    status: String,
    permission_request_id: Option<String>,
) {
    let _ = app.emit(EVENT_NAME, &ChatEvent::ToolStatusUpdate {
        tool_use_id,
        status,
        permission_request_id,
    });
}

/// Emit a tool end event (emitted from PostToolUse hook)
pub fn emit_tool_end(app: &AppHandle, id: String, status: String) {
    let _ = app.emit(EVENT_NAME, &ChatEvent::ToolEnd { id, status });
}

/// Emit a session init event
pub fn emit_session_init(app: &AppHandle, auth_type: String) {
    let _ = app.emit(EVENT_NAME, &ChatEvent::SessionInit { auth_type });
}

/// Emit a state changed event
pub fn emit_state_changed(app: &AppHandle, permission_mode: String, model: String) {
    let _ = app.emit(EVENT_NAME, &ChatEvent::StateChanged {
        permission_mode,
        model,
    });
}

/// Emit a completion event
pub fn emit_complete(app: &AppHandle) {
    let _ = app.emit(EVENT_NAME, &ChatEvent::Complete);
}

/// Emit an abort complete event
pub fn emit_abort_complete(app: &AppHandle, session_id: String) {
    let _ = app.emit(EVENT_NAME, &ChatEvent::AbortComplete { session_id });
}

/// Emit an error event
pub fn emit_error(app: &AppHandle, message: String) {
    let _ = app.emit(EVENT_NAME, &ChatEvent::Error { message });
}
