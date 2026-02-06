//! Shared runtime primitives for backend integrations.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

/// Backend-neutral event channel for chat stream updates.
pub const CHAT_EVENT_CHANNEL: &str = "chat:event";

/// Response payload for permission requests coming from the UI.
#[derive(Debug, Clone)]
pub struct PermissionResponse {
    pub allowed: bool,
}

/// Maps permission request IDs to one-shot response senders.
pub type PermissionChannels = Arc<Mutex<HashMap<String, oneshot::Sender<PermissionResponse>>>>;
