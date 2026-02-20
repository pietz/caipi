//! Claude Code backend implementation.

mod adapter;
mod event_handler;
pub mod cli_protocol;
pub mod hooks;
pub mod sessions;
pub mod settings;
pub mod tool_utils;

pub use adapter::ClaudeBackend;
