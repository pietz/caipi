//! Multi-backend abstraction layer for AI coding CLIs.
//!
//! This module provides a trait-based abstraction to support multiple AI backends
//! (Claude Code, Codex, Gemini CLI, GitHub Copilot CLI, etc.).

mod runtime;
mod session;
pub(crate) mod types;
pub(crate) mod utils;

pub mod claude;
pub mod codex;

pub use runtime::{emit_chat_event, PermissionChannels, PermissionResponse};
pub use session::BackendSession;
pub use types::{BackendError, BackendKind, BackendRegistry, ChatEvent, Message, SessionConfig, SessionStore};
