//! Multi-backend abstraction layer for AI coding CLIs.
//!
//! This module provides a trait-based abstraction to support multiple AI backends
//! (Claude Code, Codex, Gemini CLI, GitHub Copilot CLI, etc.).

mod session;
mod runtime;
mod types;

pub mod claude;

pub use session::BackendSession;
pub use runtime::{CHAT_EVENT_CHANNEL, PermissionChannels, PermissionResponse};
pub use types::{BackendError, BackendKind, BackendRegistry, SessionConfig};
