use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;

use crate::claude::agent::{AgentSession, AgentEvent, PermissionChannels, PermissionResponse};

// Global session store
pub type SessionStore = Arc<Mutex<HashMap<String, AgentSession>>>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub id: String,
    pub role: String, // "user" or "assistant"
    pub content: String,
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolActivity {
    pub id: String,
    pub tool_type: String,
    pub target: String,
    pub status: String, // "running", "completed", "error"
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum ChatEvent {
    Text { content: String },
    ToolStart { activity: ToolActivity },
    ToolEnd { id: String, status: String },
    PermissionRequest {
        id: String,
        #[serde(rename = "sessionId")]
        session_id: String,
        tool: String,
        #[serde(rename = "toolUseId")]
        tool_use_id: Option<String>,
        description: String,
    },
    SessionInit { auth_type: String },
    StateChanged {
        #[serde(rename = "permissionMode")]
        permission_mode: String,
        model: String,
    },
    Complete,
    #[serde(rename = "AbortComplete")]
    AbortComplete {
        #[serde(rename = "sessionId")]
        session_id: String,
    },
    Error { message: String },
}

#[tauri::command]
pub async fn create_session(
    folder_path: String,
    permission_mode: Option<String>,
    model: Option<String>,
    app: AppHandle,
) -> Result<String, String> {
    let sessions: tauri::State<'_, SessionStore> = app.state();
    let mode = permission_mode.unwrap_or_else(|| "default".to_string());
    let model = model.unwrap_or_else(|| "opus".to_string());
    let session = AgentSession::new(folder_path, mode, model, app.clone()).await.map_err(|e| e.to_string())?;
    let session_id = session.id.clone();

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

    // Clone the session, releasing the lock immediately
    let session = {
        let store = sessions.lock().await;
        store.get(&session_id).ok_or("Session not found")?.clone()
    };
    // Lock is now released!

    // Clone what we need for the async task
    let app_handle = app.clone();

    // Send message and stream events
    // Note: PermissionRequest events are emitted directly from the can_use_tool callback
    match session.send_message(&message, move |event| {
        let chat_event = match event {
            AgentEvent::Text(content) => ChatEvent::Text { content },
            AgentEvent::ToolStart { id, tool_type, target } => {
                ChatEvent::ToolStart {
                    activity: ToolActivity {
                        id,
                        tool_type,
                        target,
                        status: "running".to_string(),
                        timestamp: chrono::Utc::now().timestamp(),
                    },
                }
            }
            AgentEvent::ToolEnd { id, status } => ChatEvent::ToolEnd { id, status },
            AgentEvent::SessionInit { auth_type } => ChatEvent::SessionInit { auth_type },
            AgentEvent::Complete => ChatEvent::Complete,
            AgentEvent::Error(msg) => ChatEvent::Error { message: msg },
        };

        let _ = app_handle.emit("claude:event", &chat_event);
    }).await {
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

    // Clone the session, releasing the lock immediately
    let session = {
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

    // Clone the session, releasing the lock immediately
    let session = {
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

    // Clone the session, releasing the lock immediately
    let session = {
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

    // Clone the session, releasing the lock immediately
    let session = {
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

#[cfg(test)]
mod tests {
    #[test]
    fn test_session_creation() {
        // Placeholder for session lifecycle tests
        assert!(true);
    }
}
