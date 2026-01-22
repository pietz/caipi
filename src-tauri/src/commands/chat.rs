use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;

use crate::claude::agent::{AgentSession, AgentEvent};

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
    PermissionRequest { id: String, tool: String, description: String },
    Complete,
    Error { message: String },
}

#[tauri::command]
pub async fn create_session(
    folder_path: String,
    app: AppHandle,
) -> Result<String, String> {
    let sessions: tauri::State<'_, SessionStore> = app.state();
    let session = AgentSession::new(folder_path).await.map_err(|e| e.to_string())?;
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

    // Get the session
    let mut store = sessions.lock().await;
    let session = store.get_mut(&session_id).ok_or("Session not found")?;

    // Clone what we need for the async task
    let app_handle = app.clone();

    // Send message and stream events
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
            AgentEvent::PermissionRequest { id, tool, description } => {
                ChatEvent::PermissionRequest { id, tool, description }
            }
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
    session_id: String,
    request_id: String,
    allowed: bool,
    app: AppHandle,
) -> Result<(), String> {
    let sessions: tauri::State<'_, SessionStore> = app.state();
    let mut store = sessions.lock().await;
    let session = store.get_mut(&session_id).ok_or("Session not found")?;

    session
        .respond_permission(&request_id, allowed)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_session_messages(
    session_id: String,
    app: AppHandle,
) -> Result<Vec<Message>, String> {
    let sessions: tauri::State<'_, SessionStore> = app.state();
    let store = sessions.lock().await;
    let session = store.get(&session_id).ok_or("Session not found")?;

    Ok(session.messages.clone())
}
