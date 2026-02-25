//! Shared runtime primitives for backend integrations.

use crate::backends::types::ChatEvent;
use crate::backends::types::SessionStore;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
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

    if let Some(session_id) = session_id {
        if let Some(sessions) = app_handle.try_state::<SessionStore>() {
            let target_window = sessions.try_lock().ok().and_then(|store| {
                store
                    .get(session_id)
                    .map(|entry| entry.window_label.clone())
            });

            if let Some(window_label) = target_window {
                if app_handle.get_webview_window(&window_label).is_some() {
                    let _ = app_handle.emit_to(window_label, CHAT_EVENT_CHANNEL, &payload);
                    return;
                }
                log::warn!(
                    "Session {} mapped to missing window {}; falling back to broadcast",
                    session_id,
                    window_label
                );
            }
        }
    }

    let _ = app_handle.emit(CHAT_EVENT_CHANNEL, &payload);
}

/// Response payload for permission requests coming from the UI.
#[derive(Debug, Clone)]
pub struct PermissionResponse {
    pub allowed: bool,
}

/// Maps permission request IDs to one-shot response senders.
pub type PermissionChannels = Arc<Mutex<HashMap<String, oneshot::Sender<PermissionResponse>>>>;
