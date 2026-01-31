//! Multi-backend abstraction layer for AI coding CLIs.
//!
//! This module provides a trait-based abstraction to support multiple AI backends
//! (Claude Code, Codex, Gemini CLI, GitHub Copilot CLI, etc.).

mod session;
mod types;

pub mod claude;

pub use session::BackendSession;
pub use types::{
    AuthStatus, Backend, BackendCapabilities, BackendError, BackendKind, BackendRegistry,
    InstallStatus, ModelInfo, PermissionModel, SessionConfig,
};
