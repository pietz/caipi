//! Core types and traits for the backend abstraction layer.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;

use super::session::BackendSession;

/// Identifies the type of AI backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackendKind {
    Claude,
    // Future backends:
    // Codex,
    // Gemini,
    // Copilot,
}

impl std::fmt::Display for BackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendKind::Claude => write!(f, "claude"),
        }
    }
}

impl std::str::FromStr for BackendKind {
    type Err = BackendError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "claude" => Ok(BackendKind::Claude),
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

/// Information about an available model.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub supports_thinking: bool,
}

/// How the backend handles permissions.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionModel {
    /// Per-operation permission prompts (like Claude Code)
    PerOperation,
    /// Session-level permissions
    SessionLevel,
    /// No permission system
    None,
}

/// Capabilities of a backend.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendCapabilities {
    pub permission_model: PermissionModel,
    pub supports_streaming: bool,
    pub supports_abort: bool,
    pub supports_resume: bool,
    pub supports_extended_thinking: bool,
    pub available_models: Vec<ModelInfo>,
}

/// Trait for a backend implementation (e.g., Claude, Codex).
#[async_trait]
pub trait Backend: Send + Sync {
    /// Returns the kind of this backend.
    fn kind(&self) -> BackendKind;

    /// Returns the capabilities of this backend.
    #[allow(dead_code)]
    fn capabilities(&self) -> BackendCapabilities;

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

    /// Gets the default backend.
    pub fn default_backend(&self) -> Option<Arc<dyn Backend>> {
        self.get(self.default)
    }

    /// Sets the default backend kind.
    #[allow(dead_code)]
    pub fn set_default(&mut self, kind: BackendKind) {
        self.default = kind;
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
