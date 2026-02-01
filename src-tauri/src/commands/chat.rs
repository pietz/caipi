use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;

use crate::backends::BackendSession;
use crate::claude::agent::{PermissionChannels, PermissionResponse};

// Global session store - now uses Arc<dyn BackendSession> for polymorphism
pub type SessionStore = Arc<Mutex<HashMap<String, Arc<dyn BackendSession>>>>;

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
    Text { content: String },
    /// Emitted from PreToolUse hook when a tool starts
    ToolStart {
        #[serde(rename = "toolUseId")]
        tool_use_id: String,
        #[serde(rename = "toolType")]
        tool_type: String,
        target: String,
        status: String,  // "pending"
        #[serde(skip_serializing_if = "Option::is_none")]
        input: Option<serde_json::Value>,
    },
    /// Emitted when tool status changes (permission granted/denied, running, etc.)
    ToolStatusUpdate {
        #[serde(rename = "toolUseId")]
        tool_use_id: String,
        status: String,  // "awaiting_permission", "running", "denied"
        #[serde(rename = "permissionRequestId", skip_serializing_if = "Option::is_none")]
        permission_request_id: Option<String>,
    },
    /// Emitted from PostToolUse hook when a tool completes
    ToolEnd {
        id: String,
        status: String,  // "completed", "error"
    },
    SessionInit { auth_type: String },
    StateChanged {
        #[serde(rename = "permissionMode")]
        permission_mode: String,
        model: String,
    },
    TokenUsage {
        #[serde(rename = "totalTokens")]
        total_tokens: u64,
    },
    Complete,
    #[serde(rename = "AbortComplete")]
    AbortComplete {
        #[serde(rename = "sessionId")]
        session_id: String,
    },
    Error { message: String },
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
    use crate::backends::{BackendKind, BackendRegistry, SessionConfig};

    let sessions: tauri::State<'_, SessionStore> = app.state();
    let registry: tauri::State<'_, Arc<BackendRegistry>> = app.state();

    // Parse backend kind - frontend always passes the selected backend, so this default is just a safety fallback
    let backend_kind = backend
        .map(|s| s.parse::<BackendKind>())
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or(BackendKind::Claude);  // Default to Claude (matches frontend's getPersistedBackend default)

    // Get the backend from registry
    let backend_impl = registry
        .get(backend_kind)
        .ok_or_else(|| format!("Backend '{}' not available", backend_kind))?;

    // Create session config
    let config = SessionConfig {
        folder_path,
        permission_mode,
        model,
        resume_session_id,
        cli_path,
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
pub async fn send_message(
    session_id: String,
    message: String,
    app: AppHandle,
) -> Result<(), String> {
    let sessions: tauri::State<'_, SessionStore> = app.state();

    // Clone the Arc, releasing the lock immediately
    let session: Arc<dyn BackendSession> = {
        let store = sessions.lock().await;
        store.get(&session_id).ok_or("Session not found")?.clone()
    };
    // Lock is now released!

    // Send message - events are emitted via app_handle inside the session
    match session.send_message(&message).await {
        Ok(_) => Ok(()),
        Err(e) => {
            let _ = app.emit("claude:event", &ChatEvent::Error { message: e.to_string() });
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
        Err(format!("No pending permission request with id: {}", request_id))
    }
}

#[tauri::command]
pub async fn get_session_messages(
    session_id: String,
    app: AppHandle,
) -> Result<Vec<Message>, String> {
    let sessions: tauri::State<'_, SessionStore> = app.state();

    // Clone the Arc, releasing the lock immediately
    let session: Arc<dyn BackendSession> = {
        let store = sessions.lock().await;
        store.get(&session_id).ok_or("Session not found")?.clone()
    };

    Ok(session.get_messages().await)
}

#[tauri::command]
pub async fn abort_session(
    session_id: String,
    app: AppHandle,
) -> Result<(), String> {
    let sessions: tauri::State<'_, SessionStore> = app.state();

    // Clone the Arc, releasing the lock immediately
    let session: Arc<dyn BackendSession> = {
        let store = sessions.lock().await;
        store.get(&session_id).ok_or("Session not found")?.clone()
    };

    session.abort().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_permission_mode(
    session_id: String,
    mode: String,
    app: AppHandle,
) -> Result<(), String> {
    let sessions: tauri::State<'_, SessionStore> = app.state();

    // Clone the Arc, releasing the lock immediately
    let session: Arc<dyn BackendSession> = {
        let store = sessions.lock().await;
        store.get(&session_id).ok_or("Session not found")?.clone()
    };

    let result = session.set_permission_mode(mode).await;

    // Emit current state regardless of success/failure to keep frontend in sync
    let current_mode = session.get_permission_mode().await;
    let current_model = session.get_model().await;
    let _ = app.emit("claude:event", &ChatEvent::StateChanged {
        permission_mode: current_mode,
        model: current_model,
    });

    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_model(
    session_id: String,
    model: String,
    app: AppHandle,
) -> Result<(), String> {
    let sessions: tauri::State<'_, SessionStore> = app.state();

    // Clone the Arc, releasing the lock immediately
    let session: Arc<dyn BackendSession> = {
        let store = sessions.lock().await;
        store.get(&session_id).ok_or("Session not found")?.clone()
    };

    let result = session.set_model(model).await;

    // Emit current state regardless of success/failure to keep frontend in sync
    let current_mode = session.get_permission_mode().await;
    let current_model = session.get_model().await;
    let _ = app.emit("claude:event", &ChatEvent::StateChanged {
        permission_mode: current_mode,
        model: current_model,
    });

    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_thinking_level(
    session_id: String,
    level: String,
    app: AppHandle,
) -> Result<(), String> {
    let sessions: tauri::State<'_, SessionStore> = app.state();

    // Clone the Arc, releasing the lock immediately
    let session: Arc<dyn BackendSession> = {
        let store = sessions.lock().await;
        store.get(&session_id).ok_or("Session not found")?.clone()
    };

    session.set_thinking_level(level).await.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_session_creation() {
        // Placeholder for session lifecycle tests
        assert!(true);
    }
}
