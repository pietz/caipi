//! Event emission helpers for Claude chat events.
//!
//! Centralizes all event emission to ensure consistent formatting
//! and simplify the emission of various event types.

use tauri::{AppHandle, Emitter};

use super::chat::{ChatEvent, ToolActivity};

const EVENT_NAME: &str = "claude:event";

/// Emit a text event with streaming content
pub fn emit_text(app: &AppHandle, content: String) {
    let _ = app.emit(EVENT_NAME, &ChatEvent::Text { content });
}

/// Emit a tool start event
pub fn emit_tool_start(app: &AppHandle, id: String, tool_type: String, target: String) {
    let _ = app.emit(EVENT_NAME, &ChatEvent::ToolStart {
        activity: ToolActivity {
            id,
            tool_type,
            target,
            status: "running".to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        },
    });
}

/// Emit a tool end event
pub fn emit_tool_end(app: &AppHandle, id: String, status: String) {
    let _ = app.emit(EVENT_NAME, &ChatEvent::ToolEnd { id, status });
}

/// Emit a permission request event
pub fn emit_permission_request(
    app: &AppHandle,
    id: String,
    session_id: String,
    tool: String,
    tool_use_id: Option<String>,
    description: String,
) {
    let _ = app.emit(EVENT_NAME, &ChatEvent::PermissionRequest {
        id,
        session_id,
        tool,
        tool_use_id,
        description,
    });
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
