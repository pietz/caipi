use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use crate::claude::tool_utils::extract_tool_target;

#[derive(Debug, Serialize, Clone)]
pub struct SessionInfo {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "folderPath")]
    pub folder_path: String,
    #[serde(rename = "folderName")]
    pub folder_name: String,
    #[serde(rename = "firstPrompt")]
    pub first_prompt: String,
    #[serde(rename = "messageCount")]
    pub message_count: u32,
    pub created: String,
    pub modified: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ProjectSessions {
    #[serde(rename = "folderPath")]
    pub folder_path: String,
    #[serde(rename = "folderName")]
    pub folder_name: String,
    pub sessions: Vec<SessionInfo>,
    #[serde(rename = "latestModified")]
    pub latest_modified: String,
}

/// Get folder name from path
fn get_folder_name(path: &str) -> String {
    path.split('/').last().unwrap_or(path).to_string()
}

/// Verify that a project directory actually corresponds to the given folder path
/// by checking the projectPath field in sessions-index.json
fn verify_project_path(folder_path: &str, project_dir: &std::path::Path) -> bool {
    let index_path = project_dir.join("sessions-index.json");
    if !index_path.exists() {
        return false;
    }

    let content = match fs::read_to_string(&index_path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };

    // Check first entry's projectPath to verify this is the right directory
    if let Some(entries) = json.get("entries").and_then(|e| e.as_array()) {
        if let Some(first) = entries.first() {
            if let Some(stored_path) = first.get("projectPath").and_then(|p| p.as_str()) {
                return stored_path == folder_path;
            }
        }
    }

    // No entries means we can't verify, treat as not matching
    false
}

/// Read and parse sessions-index.json from a project directory
fn read_sessions_index(project_dir: &PathBuf) -> Option<Vec<SessionInfo>> {
    let index_path = project_dir.join("sessions-index.json");
    let content = fs::read_to_string(&index_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    let entries = json.get("entries")?.as_array()?;

    let sessions: Vec<SessionInfo> = entries
        .iter()
        .filter_map(|entry| {
            let session_id = entry.get("sessionId")?.as_str()?.to_string();
            // Use projectPath directly from entry (Claude stores the full path)
            let folder_path = entry.get("projectPath")?.as_str()?.to_string();
            let folder_name = get_folder_name(&folder_path);
            let first_prompt = entry.get("firstPrompt")
                .and_then(|v| v.as_str())
                .unwrap_or("No prompt")
                .to_string();
            let message_count = entry.get("messageCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let created = entry.get("created")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let modified = entry.get("modified")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Skip sessions with very low message counts or no real prompt
            if message_count < 2 {
                return None;
            }

            Some(SessionInfo {
                session_id,
                folder_path,
                folder_name,
                first_prompt,
                message_count,
                created,
                modified,
            })
        })
        .collect();

    if sessions.is_empty() {
        None
    } else {
        Some(sessions)
    }
}

#[tauri::command]
pub async fn get_all_sessions(backend: Option<String>) -> Result<Vec<ProjectSessions>, String> {
    // Delegate to get_recent_sessions with a high limit for backwards compatibility
    get_recent_sessions(1000, backend).await
}

/// Get recent sessions, filtered to existing folders and limited to top N
/// This is more efficient than get_all_sessions as it:
/// 1. Filters out non-existent folders in one pass (no IPC overhead)
/// 2. Only returns the top N sessions instead of everything
/// 3. Optionally filters by backend type
#[tauri::command]
pub async fn get_recent_sessions(limit: u32, backend: Option<String>) -> Result<Vec<ProjectSessions>, String> {
    let backend = backend.unwrap_or_else(|| "claude".to_string());

    let home_dir = dirs::home_dir().ok_or("Could not find home directory")?;

    // Different backends store sessions in different locations
    let projects_dir = match backend.as_str() {
        "claude" => home_dir.join(".claude").join("projects"),
        "codex" => home_dir.join(".codex").join("projects"),
        _ => return Err(format!("Unknown backend: {}", backend)),
    };

    if !projects_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&projects_dir)
        .map_err(|e| format!("Failed to read projects directory: {}", e))?;

    // Collect all sessions from all projects, filtering out non-existent folders
    let mut all_sessions: Vec<(SessionInfo, String)> = Vec::new(); // (session, folder_path)

    for entry in entries.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()) {
        if let Some(sessions) = read_sessions_index(&entry.path()) {
            if let Some(first) = sessions.first() {
                let folder_path = first.folder_path.clone();

                // Check if folder still exists - skip if not
                if !std::path::Path::new(&folder_path).exists() {
                    continue;
                }

                for session in sessions {
                    all_sessions.push((session, folder_path.clone()));
                }
            }
        }
    }

    // Sort all sessions by modified date (most recent first)
    all_sessions.sort_by(|a, b| b.0.modified.cmp(&a.0.modified));

    // Take top N sessions
    all_sessions.truncate(limit as usize);

    // Regroup by project, preserving order of first appearance
    let mut project_map: std::collections::HashMap<String, ProjectSessions> =
        std::collections::HashMap::new();
    let mut project_order: Vec<String> = Vec::new();

    for (session, folder_path) in all_sessions {
        if !project_map.contains_key(&folder_path) {
            project_order.push(folder_path.clone());
            project_map.insert(
                folder_path.clone(),
                ProjectSessions {
                    folder_path: folder_path.clone(),
                    folder_name: get_folder_name(&folder_path),
                    sessions: Vec::new(),
                    latest_modified: session.modified.clone(),
                },
            );
        }
        project_map.get_mut(&folder_path).unwrap().sessions.push(session);
    }

    // Convert to Vec preserving insertion order
    let projects: Vec<ProjectSessions> = project_order
        .into_iter()
        .filter_map(|path| project_map.remove(&path))
        .collect();

    Ok(projects)
}

#[tauri::command]
pub async fn get_project_sessions(folder_path: String) -> Result<Vec<SessionInfo>, String> {
    let home_dir = dirs::home_dir().ok_or("Could not find home directory")?;
    let projects_dir = home_dir.join(".claude").join("projects");

    // Encode the folder path to match Claude's format
    let encoded = folder_path.replace('/', "-");
    let project_dir = projects_dir.join(&encoded);

    if !project_dir.exists() {
        return Ok(Vec::new());
    }

    // Verify the project directory actually belongs to this folder path
    // This prevents collisions like /Users/foo-bar and /Users/foo/bar
    if !verify_project_path(&folder_path, &project_dir) {
        return Ok(Vec::new());
    }

    let mut sessions = read_sessions_index(&project_dir).unwrap_or_default();

    // Sort by modified date (most recent first)
    sessions.sort_by(|a, b| b.modified.cmp(&a.modified));

    Ok(sessions)
}

/// Tool information from session history
#[derive(Debug, Serialize, Clone)]
pub struct HistoryTool {
    pub id: String,
    #[serde(rename = "toolType")]
    pub tool_type: String,
    pub target: String,
}

/// Message from session history for display
#[derive(Debug, Serialize, Clone)]
pub struct HistoryMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: i64,
    pub tools: Vec<HistoryTool>,
}

/// Read messages from a session file for display
#[tauri::command]
pub async fn get_session_history(
    folder_path: String,
    session_id: String,
) -> Result<Vec<HistoryMessage>, String> {
    let home_dir = dirs::home_dir().ok_or("Could not find home directory")?;
    let projects_dir = home_dir.join(".claude").join("projects");

    // Encode the folder path to match Claude's format
    let encoded = folder_path.replace('/', "-");
    let project_dir = projects_dir.join(&encoded);
    let session_file = project_dir.join(format!("{}.jsonl", session_id));

    if !session_file.exists() {
        return Ok(Vec::new());
    }

    // Verify the project directory actually belongs to this folder path
    if !verify_project_path(&folder_path, &project_dir) {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&session_file)
        .map_err(|e| format!("Failed to read session file: {}", e))?;

    let mut messages: Vec<HistoryMessage> = Vec::new();
    let mut seen_uuids: HashSet<String> = HashSet::new();

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let json: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");

        // Skip non-message types and meta messages
        if msg_type != "user" && msg_type != "assistant" {
            continue;
        }

        if json.get("isMeta").and_then(|v| v.as_bool()).unwrap_or(false) {
            continue;
        }

        let message = match json.get("message") {
            Some(m) => m,
            None => continue,
        };

        let role = message.get("role").and_then(|v| v.as_str()).unwrap_or("");
        if role != "user" && role != "assistant" {
            continue;
        }

        // Deduplicate by UUID
        let uuid = json.get("uuid").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if uuid.is_empty() || seen_uuids.contains(&uuid) {
            continue;
        }
        seen_uuids.insert(uuid.clone());

        // Extract content and tools
        let mut tools: Vec<HistoryTool> = Vec::new();

        let content_str = if let Some(content_text) = message.get("content").and_then(|v| v.as_str()) {
            // Simple string content (user messages)
            content_text.to_string()
        } else if let Some(content_arr) = message.get("content").and_then(|v| v.as_array()) {
            // Array content (assistant messages with text/tool_use blocks)
            let mut text_parts: Vec<String> = Vec::new();
            for block in content_arr {
                if let Some(block_type) = block.get("type").and_then(|v| v.as_str()) {
                    match block_type {
                        "text" => {
                            if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                                text_parts.push(text.to_string());
                            }
                        }
                        "tool_use" => {
                            let tool_id = block.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let tool_name = block.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let input = block.get("input").unwrap_or(&Value::Null);
                            let target = extract_tool_target(&tool_name, input);

                            if !tool_id.is_empty() && !tool_name.is_empty() {
                                tools.push(HistoryTool {
                                    id: tool_id,
                                    tool_type: tool_name,
                                    target,
                                });
                            }
                        }
                        _ => {} // Skip thinking blocks and others
                    }
                }
            }
            text_parts.join("\n")
        } else {
            continue;
        };

        // Skip empty content or system-like messages (unless they have tools)
        if content_str.is_empty() && tools.is_empty() {
            continue;
        }
        if content_str.starts_with('<') && content_str.contains("command") {
            continue;
        }

        let timestamp_str = json.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
        let timestamp = chrono::DateTime::parse_from_rfc3339(timestamp_str)
            .map(|dt| dt.timestamp())
            .unwrap_or(0);

        messages.push(HistoryMessage {
            id: uuid,
            role: role.to_string(),
            content: content_str,
            timestamp,
            tools,
        });
    }

    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_get_folder_name() {
        assert_eq!(get_folder_name("/Users/pietz/Desktop"), "Desktop");
        assert_eq!(get_folder_name("/Users/pietz/Private/caipi"), "caipi");
        assert_eq!(get_folder_name("/Users/me/my-project"), "my-project");
    }

    #[test]
    fn test_verify_project_path_matching() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("sessions-index.json");

        let content = r#"{
            "entries": [
                {"sessionId": "abc123", "projectPath": "/Users/test/my-project"}
            ]
        }"#;
        std::fs::write(&index_path, content).unwrap();

        assert!(verify_project_path("/Users/test/my-project", temp_dir.path()));
    }

    #[test]
    fn test_verify_project_path_not_matching() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("sessions-index.json");

        let content = r#"{
            "entries": [
                {"sessionId": "abc123", "projectPath": "/Users/test/other-project"}
            ]
        }"#;
        std::fs::write(&index_path, content).unwrap();

        // This simulates the collision case: requesting /Users/test/my-project
        // but the directory contains sessions for /Users/test/other-project
        assert!(!verify_project_path("/Users/test/my-project", temp_dir.path()));
    }

    #[test]
    fn test_verify_project_path_collision_scenario() {
        // Simulate: /Users/foo-bar and /Users/foo/bar both encode to -Users-foo-bar
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("sessions-index.json");

        // Directory contains sessions for /Users/foo/bar
        let content = r#"{
            "entries": [
                {"sessionId": "abc123", "projectPath": "/Users/foo/bar"}
            ]
        }"#;
        std::fs::write(&index_path, content).unwrap();

        // Request for /Users/foo/bar should match
        assert!(verify_project_path("/Users/foo/bar", temp_dir.path()));

        // Request for /Users/foo-bar should NOT match (collision case)
        assert!(!verify_project_path("/Users/foo-bar", temp_dir.path()));
    }

    #[test]
    fn test_verify_project_path_no_index_file() {
        let temp_dir = TempDir::new().unwrap();
        // No sessions-index.json file exists
        assert!(!verify_project_path("/Users/test/project", temp_dir.path()));
    }

    #[test]
    fn test_verify_project_path_empty_entries() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("sessions-index.json");

        let content = r#"{"entries": []}"#;
        std::fs::write(&index_path, content).unwrap();

        // No entries to verify against, return false to be safe
        assert!(!verify_project_path("/Users/test/project", temp_dir.path()));
    }

    #[test]
    fn test_verify_project_path_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("sessions-index.json");

        std::fs::write(&index_path, "not valid json").unwrap();

        assert!(!verify_project_path("/Users/test/project", temp_dir.path()));
    }
}
