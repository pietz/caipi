//! Session trait for backend sessions.

use async_trait::async_trait;

use super::types::{BackendError, BackendKind};

pub const PERMISSION_MODE_DEFAULT: &str = "default";
pub const PERMISSION_MODE_ACCEPT_EDITS: &str = "acceptEdits";
pub const PERMISSION_MODE_BYPASS: &str = "bypassPermissions";

pub const VALID_PERMISSION_MODES: [&str; 3] = [
    PERMISSION_MODE_DEFAULT,
    PERMISSION_MODE_ACCEPT_EDITS,
    PERMISSION_MODE_BYPASS,
];

pub fn is_valid_permission_mode(mode: &str) -> bool {
    matches!(
        mode,
        PERMISSION_MODE_DEFAULT | PERMISSION_MODE_ACCEPT_EDITS | PERMISSION_MODE_BYPASS
    )
}

pub fn invalid_permission_mode_message(mode: &str) -> String {
    format!(
        "Invalid permission mode: {mode}. Expected one of: {}",
        VALID_PERMISSION_MODES.join(", ")
    )
}

pub fn validate_permission_mode(mode: &str) -> Result<(), BackendError> {
    if is_valid_permission_mode(mode) {
        Ok(())
    } else {
        Err(BackendError {
            message: invalid_permission_mode_message(mode),
            recoverable: true,
        })
    }
}

/// Trait for a backend session.
///
/// Sessions are created by backends and handle the actual conversation.
/// Each session wraps the backend-specific implementation (e.g., a CLI session).
#[async_trait]
pub trait BackendSession: Send + Sync {
    /// Returns the session ID.
    fn session_id(&self) -> &str;

    /// Returns the backend kind.
    #[allow(dead_code)]
    fn backend_kind(&self) -> BackendKind;

    /// Returns the folder path for this session.
    #[allow(dead_code)]
    fn folder_path(&self) -> &str;

    /// Sends a message and streams responses via the event channel.
    ///
    /// `turn_id` is an optional frontend-generated ID used for stale-event gating.
    async fn send_message(&self, message: &str, turn_id: Option<&str>) -> Result<(), BackendError>;

    /// Aborts the current operation.
    async fn abort(&self) -> Result<(), BackendError>;

    /// Cleans up the session (called on app close).
    async fn cleanup(&self);

    /// Gets the current permission mode.
    async fn get_permission_mode(&self) -> String;

    /// Validates whether a permission mode is accepted by the backend contract.
    fn validate_permission_mode_value(&self, mode: &str) -> Result<(), BackendError> {
        validate_permission_mode(mode)
    }

    /// Sets the permission mode.
    async fn set_permission_mode(&self, mode: String) -> Result<(), BackendError>;

    /// Gets the current model.
    async fn get_model(&self) -> String;

    /// Sets the model.
    async fn set_model(&self, model: String) -> Result<(), BackendError>;

    /// Sets thinking level (e.g., "off"/"on" for Claude, "low"/"medium"/"high" for Codex).
    async fn set_thinking_level(&self, level: String) -> Result<(), BackendError>;
}

#[cfg(test)]
mod tests {
    use super::{
        invalid_permission_mode_message, is_valid_permission_mode, validate_permission_mode,
        VALID_PERMISSION_MODES,
    };

    #[test]
    fn permission_mode_validation_accepts_known_modes() {
        for mode in VALID_PERMISSION_MODES {
            assert!(is_valid_permission_mode(mode));
            assert!(validate_permission_mode(mode).is_ok());
        }
    }

    #[test]
    fn permission_mode_validation_rejects_unknown_modes() {
        let invalid = "fullAccess";
        let err = validate_permission_mode(invalid).expect_err("invalid mode should fail");
        assert_eq!(err.message, invalid_permission_mode_message(invalid));
        assert!(err.recoverable);
    }
}
