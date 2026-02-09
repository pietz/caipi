use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::claude::tool_utils::extract_tool_target;
use crate::commands::chat::Message as ChatMessage;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
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

/// Get folder name from path (handles both Unix and Windows paths)
fn get_folder_name(path: &str) -> String {
    // Normalize Windows backslashes to forward slashes for cross-platform handling
    let normalized = path.replace('\\', "/");
    normalized
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(path)
        .to_string()
}

/// Encode a folder path for use as a directory name.
/// Handles both Unix (/Users/foo) and Windows (C:\Users\foo) paths.
fn encode_folder_path(path: &str) -> String {
    // Normalize path separators to forward slashes
    let normalized = path.replace('\\', "/");

    // Remove drive letter on Windows (C: -> empty)
    let without_drive = if normalized.len() >= 2 && normalized.chars().nth(1) == Some(':') {
        &normalized[2..]
    } else {
        &normalized
    };

    // Replace slashes with dashes (matching Claude's format)
    without_drive.replace('/', "-")
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
            let first_prompt = entry
                .get("firstPrompt")
                .and_then(|v| v.as_str())
                .unwrap_or("No prompt")
                .to_string();
            let message_count = entry
                .get("messageCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let created = entry
                .get("created")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let modified = entry
                .get("modified")
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
                backend: Some("claudecli".to_string()),
            })
        })
        .collect();

    if sessions.is_empty() {
        None
    } else {
        Some(sessions)
    }
}

fn parse_rfc3339_timestamp(timestamp: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|dt| dt.timestamp())
        .unwrap_or(0)
}

/// Walk directory tree collecting .jsonl files with their modification times.
fn walk_jsonl_files_with_mtime(root: &Path) -> Vec<(PathBuf, std::time::SystemTime)> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                let mtime = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                files.push((path, mtime));
            }
        }
    }
    files
}

fn walk_jsonl_files(root: &Path) -> Vec<PathBuf> {
    walk_jsonl_files_with_mtime(root)
        .into_iter()
        .map(|(path, _)| path)
        .collect()
}

fn is_hex_with_len(value: &str, len: usize) -> bool {
    value.len() == len && value.chars().all(|c| c.is_ascii_hexdigit())
}

fn trailing_uuid_like(stem: &str) -> Option<String> {
    let parts: Vec<&str> = stem.split('-').collect();
    if parts.len() < 5 {
        return None;
    }

    let tail = &parts[parts.len() - 5..];
    if is_hex_with_len(tail[0], 8)
        && is_hex_with_len(tail[1], 4)
        && is_hex_with_len(tail[2], 4)
        && is_hex_with_len(tail[3], 4)
        && is_hex_with_len(tail[4], 12)
    {
        return Some(tail.join("-"));
    }

    None
}

fn codex_session_id_from_path(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    trailing_uuid_like(stem)
}

fn read_codex_session_meta(path: &Path) -> Option<(String, String)> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    for line in reader.lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let json: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if json.get("type").and_then(|v| v.as_str()) != Some("session_meta") {
            continue;
        }
        let id = json
            .get("payload")
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str())?
            .to_string();
        let cwd = json
            .get("payload")
            .and_then(|v| v.get("cwd"))
            .and_then(|v| v.as_str())?
            .to_string();
        return Some((id, cwd));
    }

    None
}

fn codex_message_count(path: &Path) -> u32 {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return 0,
    };
    let reader = BufReader::new(file);
    let mut count: u32 = 0;

    for line in reader.lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let json: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if json.get("type").and_then(|v| v.as_str()) != Some("event_msg") {
            continue;
        }
        let payload_type = json
            .get("payload")
            .and_then(|v| v.get("type"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if payload_type == "user_message" || payload_type == "agent_message" {
            count = count.saturating_add(1);
        }
    }

    count
}

/// Fast summary: reads only the first ~20 lines to extract session_meta and first prompt.
/// Uses filesystem mtime for the `modified` field instead of scanning the entire file.
fn parse_codex_session_summary_fast(
    path: &Path,
    mtime: std::time::SystemTime,
) -> Option<SessionInfo> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut session_id = codex_session_id_from_path(path).unwrap_or_default();
    let mut folder_path = String::new();
    let mut created = String::new();
    let mut first_prompt = String::new();
    let mut lines_read = 0u32;

    for line in reader.lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        lines_read += 1;

        let json: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let timestamp = json.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
        if created.is_empty() && !timestamp.is_empty() {
            created = timestamp.to_string();
        }

        let record_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if record_type == "session_meta" {
            if let Some(id) = json
                .get("payload")
                .and_then(|v| v.get("id"))
                .and_then(|v| v.as_str())
            {
                session_id = id.to_string();
            }
            if let Some(cwd) = json
                .get("payload")
                .and_then(|v| v.get("cwd"))
                .and_then(|v| v.as_str())
            {
                folder_path = cwd.to_string();
            }
        }

        if record_type == "event_msg" && first_prompt.is_empty() {
            let payload_type = json
                .get("payload")
                .and_then(|v| v.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if payload_type == "user_message" {
                first_prompt = json
                    .get("payload")
                    .and_then(|v| v.get("message"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
            }
        }

        // Once we have both folder_path and first_prompt, stop reading
        if !folder_path.is_empty() && !first_prompt.is_empty() {
            break;
        }

        // Safety limit: don't read more than 30 lines
        if lines_read >= 30 {
            break;
        }
    }

    if session_id.is_empty() || folder_path.is_empty() {
        return None;
    }

    let folder_name = get_folder_name(&folder_path);
    if first_prompt.is_empty() {
        first_prompt = "No prompt".to_string();
    }

    // Use filesystem mtime for modified timestamp
    let modified = mtime
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .ok()
        .map(|d| {
            chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default()
        })
        .unwrap_or_default();

    Some(SessionInfo {
        session_id,
        folder_path,
        folder_name,
        first_prompt,
        message_count: codex_message_count(path),
        created: if created.is_empty() {
            modified.clone()
        } else {
            created
        },
        modified,
        backend: Some("codex".to_string()),
    })
}

fn load_codex_session_index(limit: Option<usize>) -> Result<Vec<SessionInfo>, String> {
    let home_dir = dirs::home_dir().ok_or("Could not find home directory")?;
    let sessions_root = home_dir.join(".codex").join("sessions");
    if !sessions_root.exists() {
        return Ok(Vec::new());
    }

    let mut files = walk_jsonl_files_with_mtime(&sessions_root);

    // Sort by mtime descending (most recent first)
    files.sort_by(|a, b| b.1.cmp(&a.1));

    // If a limit is given, only parse that many files
    if let Some(n) = limit {
        files.truncate(n);
    }

    let mut sessions: Vec<SessionInfo> = files
        .into_iter()
        .filter_map(|(path, mtime)| parse_codex_session_summary_fast(&path, mtime))
        .collect();

    // Already sorted by mtime from the file sort, but re-sort by the formatted string
    // to be consistent
    sessions.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(sessions)
}

fn resolve_codex_session_file(
    session_id: &str,
    folder_path: Option<&str>,
) -> Result<Option<PathBuf>, String> {
    let home_dir = dirs::home_dir().ok_or("Could not find home directory")?;
    let sessions_root = home_dir.join(".codex").join("sessions");
    if !sessions_root.exists() {
        return Ok(None);
    }

    for path in walk_jsonl_files(&sessions_root) {
        if let Some((meta_id, meta_cwd)) = read_codex_session_meta(&path) {
            let folder_matches = folder_path.map(|f| f == meta_cwd).unwrap_or(true);
            if meta_id == session_id && folder_matches {
                return Ok(Some(path));
            }
            continue;
        }

        if let Some(id) = codex_session_id_from_path(&path) {
            if id == session_id {
                return Ok(Some(path));
            }
        }
    }
    Ok(None)
}

/// Extract tool info from a Codex `response_item` payload.
/// Returns `(tool_type, target)` or `None` if the payload is not a tool event.
fn codex_tool_from_payload(payload: &Value) -> Option<(String, String)> {
    let payload_type = payload.get("type").and_then(|v| v.as_str())?;
    match payload_type {
        "function_call" => {
            let name = payload
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("command_execution");
            let tool_type = match name {
                "exec_command" => "command_execution",
                _ => name,
            };
            // Parse the arguments JSON string to extract the target
            let target = payload
                .get("arguments")
                .and_then(|v| v.as_str())
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                .and_then(|args| {
                    args.get("cmd")
                        .or_else(|| args.get("query"))
                        .or_else(|| args.get("command"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or_default();
            Some((tool_type.to_string(), target))
        }
        "web_search_call" => {
            let target = payload
                .get("action")
                .and_then(|action| {
                    action
                        .get("query")
                        .or_else(|| action.get("url"))
                        .and_then(|v| v.as_str())
                })
                .unwrap_or("")
                .to_string();
            Some(("web_search".to_string(), target))
        }
        _ => None,
    }
}

fn load_codex_history_messages(
    session_id: &str,
    folder_path: Option<&str>,
) -> Result<Vec<HistoryMessage>, String> {
    let Some(session_file) = resolve_codex_session_file(session_id, folder_path)? else {
        return Ok(Vec::new());
    };

    let file = File::open(&session_file).map_err(|e| {
        format!(
            "Failed to open Codex session file {}: {}",
            session_file.display(),
            e
        )
    })?;
    let reader = BufReader::new(file);

    let mut messages: Vec<HistoryMessage> = Vec::new();
    let mut idx = 0usize;
    let mut tool_idx = 0usize;

    for line in reader.lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let json: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let record_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let payload = match json.get("payload") {
            Some(p) => p,
            None => continue,
        };
        let payload_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");

        // Handle tool events from response_item records
        if record_type == "response_item" {
            if let Some((tool_type, target)) = codex_tool_from_payload(payload) {
                // Attach tool to the most recent assistant message
                if let Some(last_assistant) =
                    messages.iter_mut().rev().find(|m| m.role == "assistant")
                {
                    last_assistant.tools.push(HistoryTool {
                        id: format!("{}-tool-{}", session_id, tool_idx),
                        tool_type,
                        target,
                    });
                    tool_idx += 1;
                }
            }
            continue;
        }

        if record_type != "event_msg" {
            continue;
        }

        let role = match payload_type {
            "user_message" => "user",
            "agent_message" => "assistant",
            _ => continue,
        };
        let content = payload
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if content.is_empty() {
            continue;
        }
        let timestamp_str = json.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
        let timestamp = parse_rfc3339_timestamp(timestamp_str);

        messages.push(HistoryMessage {
            id: format!("{}-{}", session_id, idx),
            role: role.to_string(),
            content,
            timestamp,
            tools: Vec::new(),
        });
        idx += 1;
    }

    Ok(messages)
}

pub fn load_codex_log_messages(
    session_id: &str,
    folder_path: Option<&str>,
) -> Result<Vec<ChatMessage>, String> {
    let history = load_codex_history_messages(session_id, folder_path)?;
    Ok(history
        .into_iter()
        .map(|msg| ChatMessage {
            id: msg.id,
            role: msg.role,
            content: msg.content,
            timestamp: msg.timestamp,
        })
        .collect())
}

#[tauri::command]
pub async fn get_all_sessions(backend: Option<String>) -> Result<Vec<ProjectSessions>, String> {
    // Delegate to get_recent_sessions with a high limit for backwards compatibility
    get_recent_sessions(1000, backend).await
}

async fn get_recent_sessions_by_backend(
    limit: u32,
    backend: Option<String>,
) -> Result<Vec<ProjectSessions>, String> {
    if matches!(backend.as_deref(), Some("codex")) {
        let mut all_sessions: Vec<(SessionInfo, String)> =
            load_codex_session_index(Some(limit as usize))?
                .into_iter()
                .filter_map(|session| {
                    let folder_path = session.folder_path.clone();
                    if !std::path::Path::new(&folder_path).exists() {
                        return None;
                    }
                    Some((session, folder_path))
                })
                .collect();

        all_sessions.sort_by(|a, b| b.0.modified.cmp(&a.0.modified));
        all_sessions.truncate(limit as usize);

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
            project_map
                .get_mut(&folder_path)
                .unwrap()
                .sessions
                .push(session);
        }

        return Ok(project_order
            .into_iter()
            .filter_map(|path| project_map.remove(&path))
            .collect());
    }

    let home_dir = dirs::home_dir().ok_or("Could not find home directory")?;
    let projects_dir = home_dir.join(".claude").join("projects");

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
        project_map
            .get_mut(&folder_path)
            .unwrap()
            .sessions
            .push(session);
    }

    // Convert to Vec preserving insertion order
    let projects: Vec<ProjectSessions> = project_order
        .into_iter()
        .filter_map(|path| project_map.remove(&path))
        .collect();

    Ok(projects)
}

#[tauri::command]
pub async fn get_recent_sessions(
    limit: u32,
    backend: Option<String>,
) -> Result<Vec<ProjectSessions>, String> {
    get_recent_sessions_by_backend(limit, backend).await
}

#[tauri::command]
pub async fn get_project_sessions(
    folder_path: String,
    backend: Option<String>,
) -> Result<Vec<SessionInfo>, String> {
    if matches!(backend.as_deref(), Some("codex")) {
        if !std::path::Path::new(&folder_path).exists() {
            return Ok(Vec::new());
        }
        let mut sessions: Vec<SessionInfo> = load_codex_session_index(None)?
            .into_iter()
            .filter(|s| s.folder_path == folder_path)
            .collect();
        sessions.sort_by(|a, b| b.modified.cmp(&a.modified));
        return Ok(sessions);
    }

    let home_dir = dirs::home_dir().ok_or("Could not find home directory")?;
    let projects_dir = home_dir.join(".claude").join("projects");

    // Encode the folder path to match Claude's format
    let encoded = encode_folder_path(&folder_path);
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

fn resolve_session_file(folder_path: &str, session_id: &str) -> Result<Option<PathBuf>, String> {
    let home_dir = dirs::home_dir().ok_or("Could not find home directory")?;
    let projects_dir = home_dir.join(".claude").join("projects");

    // Encode the folder path to match Claude's format
    let encoded = encode_folder_path(folder_path);
    let project_dir = projects_dir.join(&encoded);
    let session_file = project_dir.join(format!("{}.jsonl", session_id));

    if !session_file.exists() {
        return Ok(None);
    }

    // Verify the project directory actually belongs to this folder path
    if !verify_project_path(folder_path, &project_dir) {
        return Ok(None);
    }

    Ok(Some(session_file))
}

/// Load parsed history messages from a Claude session log file.
pub fn load_session_history_messages(
    folder_path: &str,
    session_id: &str,
) -> Result<Vec<HistoryMessage>, String> {
    let Some(session_file) = resolve_session_file(folder_path, session_id)? else {
        return Ok(Vec::new());
    };

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

        if json
            .get("isMeta")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
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
        let uuid = json
            .get("uuid")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if uuid.is_empty() || seen_uuids.contains(&uuid) {
            continue;
        }
        seen_uuids.insert(uuid.clone());

        // Extract content and tools
        let mut tools: Vec<HistoryTool> = Vec::new();

        let content_str =
            if let Some(content_text) = message.get("content").and_then(|v| v.as_str()) {
                // Simple string content (user messages)
                content_text.to_string()
            } else if let Some(content_arr) = message.get("content").and_then(|v| v.as_array()) {
                // Array content (assistant messages with text/tool_use blocks)
                let mut text_parts: Vec<String> = Vec::new();
                let mut thinking_idx: usize = 0;
                for block in content_arr {
                    if let Some(block_type) = block.get("type").and_then(|v| v.as_str()) {
                        match block_type {
                            "text" => {
                                if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                                    text_parts.push(text.to_string());
                                }
                            }
                            "tool_use" => {
                                let tool_id = block
                                    .get("id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let tool_name = block
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
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
                            "thinking" => {
                                if let Some(thinking_content) =
                                    block.get("thinking").and_then(|v| v.as_str())
                                {
                                    // Make thinking tool IDs stable and unique across merged history messages.
                                    let thinking_id = format!("{}-thinking-{}", uuid, thinking_idx);
                                    thinking_idx += 1;
                                    tools.push(HistoryTool {
                                        id: thinking_id,
                                        tool_type: "Thinking".to_string(),
                                        target: thinking_content.to_string(),
                                    });
                                }
                            }
                            _ => {}
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

/// Load log messages into the in-memory session shape used by chat sessions.
pub fn load_session_log_messages(
    folder_path: &str,
    session_id: &str,
) -> Result<Vec<ChatMessage>, String> {
    let history = load_session_history_messages(folder_path, session_id)?;
    Ok(history
        .into_iter()
        .map(|msg| ChatMessage {
            id: msg.id,
            role: msg.role,
            content: msg.content,
            timestamp: msg.timestamp,
        })
        .collect())
}

/// Read messages from a session file for display
#[tauri::command]
pub async fn get_session_history(
    folder_path: String,
    session_id: String,
    backend: Option<String>,
) -> Result<Vec<HistoryMessage>, String> {
    if matches!(backend.as_deref(), Some("codex")) {
        return load_codex_history_messages(&session_id, Some(folder_path.as_str()));
    }
    load_session_history_messages(&folder_path, &session_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_get_folder_name_unix() {
        assert_eq!(get_folder_name("/Users/pietz/Desktop"), "Desktop");
        assert_eq!(get_folder_name("/Users/pietz/Private/caipi"), "caipi");
        assert_eq!(get_folder_name("/Users/me/my-project"), "my-project");
    }

    #[test]
    fn test_get_folder_name_windows() {
        assert_eq!(get_folder_name(r"C:\Users\pietz\Desktop"), "Desktop");
        assert_eq!(get_folder_name(r"D:\Projects\my-app"), "my-app");
        assert_eq!(get_folder_name(r"C:\Users\me\Documents\code"), "code");
    }

    #[test]
    fn test_encode_folder_path_unix() {
        assert_eq!(encode_folder_path("/Users/foo/bar"), "-Users-foo-bar");
        assert_eq!(
            encode_folder_path("/home/user/projects"),
            "-home-user-projects"
        );
    }

    #[test]
    fn test_encode_folder_path_windows() {
        assert_eq!(encode_folder_path(r"C:\Users\foo\bar"), "-Users-foo-bar");
        assert_eq!(encode_folder_path(r"D:\Projects"), "-Projects");
        assert_eq!(
            encode_folder_path(r"C:\Users\me\Documents"),
            "-Users-me-Documents"
        );
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

        assert!(verify_project_path(
            "/Users/test/my-project",
            temp_dir.path()
        ));
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
        assert!(!verify_project_path(
            "/Users/test/my-project",
            temp_dir.path()
        ));
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

    #[test]
    fn test_codex_session_id_from_path_extracts_full_uuid() {
        let path = Path::new(
            "/tmp/rollout-2025-11-03T21-20-16-019a4b60-a492-7f12-9abe-73797723f5b1.jsonl",
        );
        assert_eq!(
            codex_session_id_from_path(path).as_deref(),
            Some("019a4b60-a492-7f12-9abe-73797723f5b1")
        );
    }

    #[test]
    fn test_codex_session_id_from_path_none_without_uuid() {
        let path = Path::new("/tmp/not-a-codex-session.jsonl");
        assert!(codex_session_id_from_path(path).is_none());
    }

    #[test]
    fn test_codex_message_count_counts_user_and_assistant_messages() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("session.jsonl");
        let content = r#"{"type":"session_meta","payload":{"id":"abc","cwd":"/tmp/project"}}
{"type":"event_msg","payload":{"type":"user_message","message":"hello"},"timestamp":"2026-01-01T00:00:00Z"}
{"type":"event_msg","payload":{"type":"agent_message","message":"hi"},"timestamp":"2026-01-01T00:00:01Z"}
{"type":"event_msg","payload":{"type":"tool_call","message":"ignored"},"timestamp":"2026-01-01T00:00:02Z"}"#;
        std::fs::write(&path, content).unwrap();

        assert_eq!(codex_message_count(&path), 2);
    }
}
