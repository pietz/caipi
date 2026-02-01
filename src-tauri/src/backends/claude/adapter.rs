//! Claude Code backend adapter.
//!
//! This module wraps the existing AgentSession to implement the Backend and BackendSession traits.

use async_trait::async_trait;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

use crate::backends::session::BackendSession;
use crate::backends::types::{
    AuthStatus, Backend, BackendCapabilities, BackendError, BackendKind, InstallStatus, ModelInfo,
    PermissionModel, SessionConfig,
};
use crate::claude::agent::{AgentEvent, AgentSession};
use crate::commands::chat::{ChatEvent, Message};
use crate::commands::setup::{check_cli_authenticated_internal, check_cli_installed_internal};

/// Claude Code backend implementation.
pub struct ClaudeBackend;

impl ClaudeBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClaudeBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Backend for ClaudeBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Claude
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            permission_model: PermissionModel::PerOperation,
            supports_streaming: true,
            supports_abort: true,
            supports_resume: true,
            supports_extended_thinking: true,
            available_models: vec![
                ModelInfo {
                    id: "opus".to_string(),
                    name: "Claude Opus 4.5".to_string(),
                    supports_thinking: true,
                },
                ModelInfo {
                    id: "sonnet".to_string(),
                    name: "Claude Sonnet 4.5".to_string(),
                    supports_thinking: true,
                },
                ModelInfo {
                    id: "haiku".to_string(),
                    name: "Claude Haiku 4.5".to_string(),
                    supports_thinking: false,
                },
            ],
        }
    }

    async fn check_installed(&self) -> Result<InstallStatus, BackendError> {
        let status = check_cli_installed_internal().await;
        Ok(InstallStatus {
            installed: status.installed,
            version: status.version,
            path: status.path,
        })
    }

    async fn check_authenticated(&self) -> Result<AuthStatus, BackendError> {
        let status = check_cli_authenticated_internal().await;
        Ok(AuthStatus {
            authenticated: status.authenticated,
        })
    }

    async fn create_session(
        &self,
        config: SessionConfig,
        app_handle: AppHandle,
    ) -> Result<Arc<dyn BackendSession>, BackendError> {
        let permission_mode = config.permission_mode.unwrap_or_else(|| "default".to_string());
        let model = config.model.unwrap_or_else(|| "sonnet".to_string());

        let session = AgentSession::new(
            config.folder_path,
            permission_mode,
            model,
            config.resume_session_id,
            config.cli_path,
            app_handle.clone(),
        )
        .await
        .map_err(|e| BackendError {
            message: e.to_string(),
            recoverable: false,
        })?;

        Ok(Arc::new(ClaudeSession::new(session, app_handle)))
    }
}

/// Claude session wrapper implementing BackendSession.
pub struct ClaudeSession {
    inner: AgentSession,
    app_handle: AppHandle,
}

impl ClaudeSession {
    pub fn new(session: AgentSession, app_handle: AppHandle) -> Self {
        Self {
            inner: session,
            app_handle,
        }
    }
}

#[async_trait]
impl BackendSession for ClaudeSession {
    fn session_id(&self) -> &str {
        &self.inner.id
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::Claude
    }

    fn folder_path(&self) -> &str {
        &self.inner.folder_path
    }

    async fn send_message(&self, message: &str) -> Result<(), BackendError> {
        // Clone app_handle for the callback closure
        let app_handle = self.app_handle.clone();

        self.inner
            .send_message(message, move |event| {
                // Convert AgentEvent to ChatEvent and emit
                let chat_event = match event {
                    AgentEvent::Text(content) => ChatEvent::Text { content },
                    AgentEvent::SessionInit { auth_type } => ChatEvent::SessionInit { auth_type },
                    AgentEvent::TokenUsage { total_tokens } => {
                        ChatEvent::TokenUsage { total_tokens }
                    }
                    AgentEvent::Complete => ChatEvent::Complete,
                    AgentEvent::Error(msg) => ChatEvent::Error { message: msg },
                };
                let _ = app_handle.emit("claude:event", &chat_event);
            })
            .await
            .map_err(|e| BackendError {
                message: e.to_string(),
                recoverable: false,
            })
    }

    async fn abort(&self) -> Result<(), BackendError> {
        self.inner.abort().await.map_err(|e| BackendError {
            message: e.to_string(),
            recoverable: false,
        })
    }

    async fn cleanup(&self) {
        self.inner.cleanup().await;
    }

    async fn get_permission_mode(&self) -> String {
        self.inner.get_permission_mode().await
    }

    async fn set_permission_mode(&self, mode: String) -> Result<(), BackendError> {
        self.inner
            .set_permission_mode(mode)
            .await
            .map_err(|e| BackendError {
                message: e.to_string(),
                recoverable: false,
            })
    }

    async fn get_model(&self) -> String {
        self.inner.get_model().await
    }

    async fn set_model(&self, model: String) -> Result<(), BackendError> {
        self.inner.set_model(model).await.map_err(|e| BackendError {
            message: e.to_string(),
            recoverable: false,
        })
    }

    async fn set_thinking_level(&self, level: String) -> Result<(), BackendError> {
        // Map string level to boolean: "on" -> true, everything else -> false
        let enabled = level == "on";
        self.inner
            .set_extended_thinking(enabled)
            .await
            .map_err(|e| BackendError {
                message: e.to_string(),
                recoverable: false,
            })
    }

    async fn get_messages(&self) -> Vec<Message> {
        self.inner.get_messages().await
    }
}
