//! Core types and traits for the backend abstraction layer.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::Mutex;

use super::session::BackendSession;

// ---------------------------------------------------------------------------
// Domain types (moved from commands/chat.rs)
// ---------------------------------------------------------------------------

/// Session entry tracked by the runtime.
///
/// Keeping `session` and `window_label` together ensures ownership remains
/// consistent across create, routing, and close cleanup paths.
pub struct SessionRecord {
    pub session: Arc<dyn BackendSession>,
    pub window_label: String,
}

/// Global session store.
pub type SessionStore = Arc<Mutex<HashMap<String, SessionRecord>>>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub id: String,
    pub role: String, // "user" or "assistant"
    pub content: String,
    pub timestamp: i64,
}

impl Message {
    /// Create a new message with a generated UUID and current timestamp.
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: role.into(),
            content: content.into(),
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum ChatEvent {
    Text {
        content: String,
    },
    /// Emitted from PreToolUse hook when a tool starts
    ToolStart {
        #[serde(rename = "toolUseId")]
        tool_use_id: String,
        #[serde(rename = "toolType")]
        tool_type: String,
        target: String,
        status: String, // "pending"
        #[serde(skip_serializing_if = "Option::is_none")]
        input: Option<serde_json::Value>,
    },
    /// Emitted when tool status changes (permission granted/denied, running, etc.)
    ToolStatusUpdate {
        #[serde(rename = "toolUseId")]
        tool_use_id: String,
        status: String, // "awaiting_permission", "running", "denied"
        #[serde(
            rename = "permissionRequestId",
            skip_serializing_if = "Option::is_none"
        )]
        permission_request_id: Option<String>,
    },
    /// Emitted from PostToolUse hook when a tool completes
    ToolEnd {
        id: String,
        status: String, // "completed", "error"
    },
    SessionInit {
        auth_type: String,
    },
    StateChanged {
        #[serde(rename = "permissionMode")]
        permission_mode: String,
        model: String,
    },
    TokenUsage {
        #[serde(rename = "totalTokens")]
        total_tokens: u64,
        #[serde(rename = "contextTokens", skip_serializing_if = "Option::is_none")]
        context_tokens: Option<u64>,
        #[serde(rename = "contextWindow", skip_serializing_if = "Option::is_none")]
        context_window: Option<u64>,
    },
    Complete,
    #[serde(rename = "AbortComplete")]
    AbortComplete {
        #[serde(rename = "sessionId")]
        session_id: String,
    },
    Error {
        message: String,
    },
    ThinkingStart {
        #[serde(rename = "thinkingId")]
        thinking_id: String,
        content: String,
    },
    ThinkingEnd {
        #[serde(rename = "thinkingId")]
        thinking_id: String,
    },
}

/// Identifies the type of AI backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackendKind {
    /// Claude CLI direct wrapper (spawns `claude` CLI directly)
    Claude,
    /// Codex CLI direct wrapper (spawns codex CLI directly)
    Codex,
    // Future backends:
    // Gemini,
    // Copilot,
}

impl std::fmt::Display for BackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendKind::Claude => write!(f, "claude"),
            BackendKind::Codex => write!(f, "codex"),
        }
    }
}

impl std::str::FromStr for BackendKind {
    type Err = BackendError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            // Backwards compatibility:
            // - "claude" previously referred to an SDK-backed backend (now removed)
            // - "claudecli" was the CLI-backed backend (now renamed to "claude")
            "claude" | "claudecli" => Ok(BackendKind::Claude),
            "codex" => Ok(BackendKind::Codex),
            _ => Err(BackendError {
                message: format!("Unknown backend: {}", s),
                recoverable: false,
            }),
        }
    }
}

/// Error type for backend operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendError {
    pub message: String,
    pub recoverable: bool,
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for BackendError {}

/// Status of CLI installation.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
}

/// Status of CLI authentication.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStatus {
    pub authenticated: bool,
}

/// Configuration for creating a new session.
#[derive(Debug, Clone, Default)]
pub struct SessionConfig {
    pub folder_path: String,
    pub permission_mode: Option<String>,
    pub model: Option<String>,
    pub resume_session_id: Option<String>,
    pub cli_path: Option<String>,
}

/// Trait for a backend implementation (e.g., Claude, Codex).
#[async_trait]
pub trait Backend: Send + Sync {
    /// Returns the kind of this backend.
    fn kind(&self) -> BackendKind;

    /// Checks if the CLI is installed.
    #[allow(dead_code)]
    async fn check_installed(&self) -> Result<InstallStatus, BackendError>;

    /// Checks if the CLI is authenticated.
    #[allow(dead_code)]
    async fn check_authenticated(&self) -> Result<AuthStatus, BackendError>;

    /// Creates a new session.
    async fn create_session(
        &self,
        config: SessionConfig,
        app_handle: AppHandle,
    ) -> Result<Arc<dyn BackendSession>, BackendError>;
}

/// Registry of available backends.
pub struct BackendRegistry {
    backends: HashMap<BackendKind, Arc<dyn Backend>>,
    default: BackendKind,
}

impl BackendRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self {
            backends: HashMap::new(),
            default: BackendKind::Claude,
        }
    }

    /// Registers a backend.
    pub fn register(&mut self, backend: Arc<dyn Backend>) {
        self.backends.insert(backend.kind(), backend);
    }

    /// Gets a backend by kind.
    pub fn get(&self, kind: BackendKind) -> Option<Arc<dyn Backend>> {
        self.backends.get(&kind).cloned()
    }

    /// Sets the default backend kind.
    #[allow(dead_code)]
    pub fn set_default(&mut self, kind: BackendKind) {
        self.default = kind;
    }

    /// Gets the default backend kind.
    pub fn default_kind(&self) -> BackendKind {
        self.default
    }

    /// Returns all registered backend kinds.
    #[allow(dead_code)]
    pub fn available_backends(&self) -> Vec<BackendKind> {
        self.backends.keys().copied().collect()
    }
}

impl Default for BackendRegistry {
    fn default() -> Self {
        Self::new()
    }
}
