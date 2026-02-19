use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use crate::backends::{
    emit_chat_event, BackendRegistry, BackendSession, PermissionChannels, PermissionResponse,
    SessionConfig,
};
use crate::storage;

// Global session store - now uses Arc<dyn BackendSession> for multi-backend support
pub type SessionStore = Arc<Mutex<HashMap<String, Arc<dyn BackendSession>>>>;

async fn get_session_from_store(
    sessions: &SessionStore,
    session_id: &str,
) -> Result<Arc<dyn BackendSession>, String> {
    let store = sessions.lock().await;
    store
        .get(session_id)
        .cloned()
        .ok_or("Session not found".to_string())
}

async fn remove_session_from_store(
    sessions: &SessionStore,
    session_id: &str,
) -> Option<Arc<dyn BackendSession>> {
    let mut store = sessions.lock().await;
    store.remove(session_id)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub id: String,
    pub role: String, // "user" or "assistant"
    pub content: String,
    pub timestamp: i64,
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

#[tauri::command]
pub async fn create_session(
    folder_path: String,
    permission_mode: Option<String>,
    model: Option<String>,
    resume_session_id: Option<String>,
    cli_path: Option<String>,
    backend: Option<String>,
    app: AppHandle,
) -> Result<String, String> {
    let sessions: tauri::State<'_, SessionStore> = app.state();
    let registry: tauri::State<'_, Arc<BackendRegistry>> = app.state();

    let selected_backend = if let Some(backend_name) = backend {
        backend_name
    } else {
        let persisted_default = storage::get_default_backend().map_err(|e| e.to_string())?;
        match persisted_default
            .as_deref()
            .and_then(|name| name.parse::<crate::backends::BackendKind>().ok())
        {
            Some(kind) => kind.to_string(),
            None => registry.default_kind().to_string(),
        }
    };

    let kind: crate::backends::BackendKind = selected_backend
        .parse()
        .map_err(|e: crate::backends::BackendError| e.to_string())?;
    let backend_name = kind.to_string();
    let backend_impl = registry
        .get(kind)
        .ok_or_else(|| format!("Backend not registered: {}", backend_name))?;

    let resolved_cli_path = match cli_path {
        Some(path) => Some(path),
        None => {
            let stored = storage::get_backend_cli_path(&backend_name).map_err(|e| e.to_string())?;
            match stored {
                Some(path) => Some(path),
                None => {
                    // No path stored — run detection to find the binary.
                    // Tauri apps don't inherit the user's shell PATH, so bare
                    // binary names like "codex" won't resolve.
                    let status = crate::commands::setup::check_backend_cli_installed_internal(&backend_name).await;
                    status.path
                }
            }
        }
    };

    // Create session config
    let config = SessionConfig {
        folder_path,
        permission_mode,
        model,
        resume_session_id,
        cli_path: resolved_cli_path,
    };

    // Create session via backend
    let session = backend_impl
        .create_session(config, app.clone())
        .await
        .map_err(|e| e.to_string())?;

    let session_id = session.session_id().to_string();

    let mut store = sessions.lock().await;
    store.insert(session_id.clone(), session);

    Ok(session_id)
}

#[tauri::command]
pub async fn destroy_session(session_id: String, app: AppHandle) -> Result<(), String> {
    let sessions: tauri::State<'_, SessionStore> = app.state();

    // Remove session from store and get it for cleanup
    let session = remove_session_from_store(&sessions, &session_id).await;

    // Clean up the session if it existed
    if let Some(session) = session {
        session.cleanup().await;
    }

    Ok(())
}

#[tauri::command]
pub async fn send_message(
    session_id: String,
    message: String,
    turn_id: Option<String>,
    app: AppHandle,
) -> Result<(), String> {
    let sessions: tauri::State<'_, SessionStore> = app.state();

    // Get the session Arc, releasing the lock immediately
    let session = get_session_from_store(&sessions, &session_id).await?;
    // Lock is now released!

    // Send message via the backend session
    match session.send_message(&message, turn_id.as_deref()).await {
        Ok(_) => Ok(()),
        Err(e) => {
            let error_event = ChatEvent::Error {
                message: e.to_string(),
            };
            emit_chat_event(
                &app,
                Some(session_id.as_str()),
                turn_id.as_deref(),
                &error_event,
            );
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub async fn respond_permission(
    _session_id: String,
    request_id: String,
    allowed: bool,
    app: AppHandle,
) -> Result<(), String> {
    // Use the separate permission channels to avoid deadlock with session store
    let permission_channels: tauri::State<'_, PermissionChannels> = app.state();

    // Take the sender from the channels map
    let sender = {
        let mut channels = permission_channels.lock().await;
        channels.remove(&request_id)
    };

    if let Some(tx) = sender {
        let _ = tx.send(PermissionResponse { allowed });
        Ok(())
    } else {
        Err(format!(
            "No pending permission request with id: {}",
            request_id
        ))
    }
}

#[tauri::command]
pub async fn abort_session(session_id: String, app: AppHandle) -> Result<(), String> {
    let sessions: tauri::State<'_, SessionStore> = app.state();

    // Get the session Arc, releasing the lock immediately
    let session = get_session_from_store(&sessions, &session_id).await?;

    session.abort().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_permission_mode(
    session_id: String,
    mode: String,
    app: AppHandle,
) -> Result<(), String> {
    let sessions: tauri::State<'_, SessionStore> = app.state();

    // Get the session Arc, releasing the lock immediately
    let session = get_session_from_store(&sessions, &session_id).await?;

    let result = session.set_permission_mode(mode).await;

    // Emit current state regardless of success/failure to keep frontend in sync
    let current_mode = session.get_permission_mode().await;
    let current_model = session.get_model().await;
    let state_event = ChatEvent::StateChanged {
        permission_mode: current_mode,
        model: current_model,
    };
    emit_chat_event(&app, Some(session_id.as_str()), None, &state_event);

    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_model(session_id: String, model: String, app: AppHandle) -> Result<(), String> {
    let sessions: tauri::State<'_, SessionStore> = app.state();

    // Get the session Arc, releasing the lock immediately
    let session = get_session_from_store(&sessions, &session_id).await?;

    let result = session.set_model(model).await;

    // Emit current state regardless of success/failure to keep frontend in sync
    let current_mode = session.get_permission_mode().await;
    let current_model = session.get_model().await;
    let state_event = ChatEvent::StateChanged {
        permission_mode: current_mode,
        model: current_model,
    };
    emit_chat_event(&app, Some(session_id.as_str()), None, &state_event);

    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_thinking_level(
    session_id: String,
    level: String,
    app: AppHandle,
) -> Result<(), String> {
    let sessions: tauri::State<'_, SessionStore> = app.state();

    // Get the session Arc, releasing the lock immediately
    let session = get_session_from_store(&sessions, &session_id).await?;

    session
        .set_thinking_level(level)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        get_session_from_store, remove_session_from_store, BackendSession, SessionStore,
    };
    use crate::backends::{BackendError, BackendKind};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    struct TestSession {
        id: String,
    }

    impl TestSession {
        fn new(id: &str) -> Self {
            Self { id: id.to_string() }
        }
    }

    #[async_trait]
    impl BackendSession for TestSession {
        fn session_id(&self) -> &str {
            &self.id
        }

        fn backend_kind(&self) -> BackendKind {
            BackendKind::Claude
        }

        fn folder_path(&self) -> &str {
            "/tmp"
        }

        async fn send_message(
            &self,
            _message: &str,
            _turn_id: Option<&str>,
        ) -> Result<(), BackendError> {
            Ok(())
        }

        async fn abort(&self) -> Result<(), BackendError> {
            Ok(())
        }

        async fn cleanup(&self) {}

        async fn get_permission_mode(&self) -> String {
            "default".to_string()
        }

        async fn set_permission_mode(&self, _mode: String) -> Result<(), BackendError> {
            Ok(())
        }

        async fn get_model(&self) -> String {
            "sonnet".to_string()
        }

        async fn set_model(&self, _model: String) -> Result<(), BackendError> {
            Ok(())
        }

        async fn set_thinking_level(&self, _level: String) -> Result<(), BackendError> {
            Ok(())
        }
    }

    fn test_store() -> SessionStore {
        Arc::new(Mutex::new(HashMap::new()))
    }

    #[tokio::test]
    async fn get_session_from_store_returns_existing_session() {
        let sessions = test_store();
        let session: Arc<dyn BackendSession> = Arc::new(TestSession::new("session-1"));
        sessions
            .lock()
            .await
            .insert("session-1".to_string(), session);

        let found = get_session_from_store(&sessions, "session-1")
            .await
            .unwrap();
        assert_eq!(found.session_id(), "session-1");
    }

    #[tokio::test]
    async fn get_session_from_store_returns_error_when_missing() {
        let sessions = test_store();
        let result = get_session_from_store(&sessions, "missing").await;
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), "Session not found");
    }

    #[tokio::test]
    async fn remove_session_from_store_removes_and_returns_session() {
        let sessions = test_store();
        let session: Arc<dyn BackendSession> = Arc::new(TestSession::new("session-2"));
        sessions
            .lock()
            .await
            .insert("session-2".to_string(), session);

        let removed = remove_session_from_store(&sessions, "session-2").await;
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().session_id(), "session-2");
        assert!(sessions.lock().await.is_empty());
    }

    #[test]
    fn test_session_creation() {
        let sessions = test_store();
        assert!(Arc::strong_count(&sessions) >= 1);
    }
}
