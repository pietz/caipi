//! Codex CLI backend implementation.

mod adapter;
mod cli_protocol;
mod event_handler;
pub mod sessions;
pub mod tool_utils;

pub use adapter::CodexBackend;
