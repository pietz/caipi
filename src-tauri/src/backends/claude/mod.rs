//! Claude Code backend implementations.

mod adapter;
mod cli_adapter;

pub use adapter::ClaudeBackend;
pub use cli_adapter::ClaudeCliBackend;
