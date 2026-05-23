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

const APP_RESPONSE_STYLE_INSTRUCTIONS: &str = concat!(
    "App formatting requirements:\n",
    "- When providing shell commands, always output directly copy-pasteable commands in fenced ```bash blocks.\n",
    "- Keep shell commands on a single line unless a multiline form is still directly copy-pasteable.\n",
    "- Do not mix explanatory prose into command blocks.\n",
    "- Avoid markdown tables unless the information is genuinely tabular and a table is clearly the most readable format.\n",
);

pub(crate) fn apply_app_response_style_instructions(user_message: &str) -> String {
    format!(
        "{instructions}\nUser request:\n{user_message}",
        instructions = APP_RESPONSE_STYLE_INSTRUCTIONS
    )
}

pub use runtime::{emit_chat_event, PermissionChannels, PermissionResponse};
pub use session::BackendSession;
pub use types::{
    BackendError, BackendKind, BackendRegistry, ChatEvent, Message, SessionConfig, SessionRecord,
    SessionStore,
};
