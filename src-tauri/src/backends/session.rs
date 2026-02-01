//! Session trait for backend sessions.

use async_trait::async_trait;

use super::types::{BackendError, BackendKind};
use crate::commands::chat::Message;

/// Trait for a backend session.
///
/// Sessions are created by backends and handle the actual conversation.
/// Each session wraps the backend-specific implementation (e.g., AgentSession for Claude).
#[async_trait]
pub trait BackendSession: Send + Sync {
    /// Returns the session ID.
    fn session_id(&self) -> &str;

    /// Returns the backend kind.
    fn backend_kind(&self) -> BackendKind;

    /// Returns the folder path for this session.
    fn folder_path(&self) -> &str;

    /// Sends a message and streams responses via the event channel.
    async fn send_message(&self, message: &str) -> Result<(), BackendError>;

    /// Aborts the current operation.
    async fn abort(&self) -> Result<(), BackendError>;

    /// Cleans up the session (called on app close).
    async fn cleanup(&self);

    /// Gets the current permission mode.
    async fn get_permission_mode(&self) -> String;

    /// Sets the permission mode.
    async fn set_permission_mode(&self, mode: String) -> Result<(), BackendError>;

    /// Gets the current model.
    async fn get_model(&self) -> String;

    /// Sets the model.
    async fn set_model(&self, model: String) -> Result<(), BackendError>;

    /// Sets thinking level (e.g., "off"/"on" for Claude, "low"/"medium"/"high" for Codex).
    async fn set_thinking_level(&self, level: String) -> Result<(), BackendError>;

    /// Gets the messages in this session.
    async fn get_messages(&self) -> Vec<Message>;
}
