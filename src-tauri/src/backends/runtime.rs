//! Shared runtime primitives for backend integrations.

use crate::commands::chat::ChatEvent;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{oneshot, Mutex};

/// Backend-neutral event channel for chat stream updates.
pub const CHAT_EVENT_CHANNEL: &str = "chat:event";

/// Metadata wrapper for chat events so the frontend can reject stale session/turn events.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatEventEnvelope<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<&'a str>,
    #[serde(flatten)]
    pub event: &'a ChatEvent,
}

/// Emit a chat event with optional session/turn metadata.
pub fn emit_chat_event(
    app_handle: &AppHandle,
    session_id: Option<&str>,
    turn_id: Option<&str>,
    event: &ChatEvent,
) {
    let payload = ChatEventEnvelope {
        session_id,
        turn_id,
        event,
    };
    let _ = app_handle.emit(CHAT_EVENT_CHANNEL, &payload);
}

/// Response payload for permission requests coming from the UI.
#[derive(Debug, Clone)]
pub struct PermissionResponse {
    pub allowed: bool,
}

/// Maps permission request IDs to one-shot response senders.
pub type PermissionChannels = Arc<Mutex<HashMap<String, oneshot::Sender<PermissionResponse>>>>;
