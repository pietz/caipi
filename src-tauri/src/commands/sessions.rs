use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::backends::claude::sessions::{
    load_claude_session_index, load_session_history_messages, parse_claude_session_summary_fast,
};
use crate::backends::codex::sessions::{
    load_codex_history_messages, load_codex_session_index, load_recent_codex_sessions,
};
use crate::commands::chat::Message as ChatMessage;

#[derive(Debug, Serialize, Deserialize, Clone)]
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

/// Session index cache: stores parsed session summaries keyed by (file_path, mtime_secs).
/// Avoids re-parsing unchanged files across app launches.
#[derive(Debug, Serialize, Deserialize, Default)]
pub(crate) struct SessionIndexCache {
    /// Map from file path string to (mtime_secs, SessionInfo)
    entries: HashMap<String, (u64, SessionInfo)>,
}

impl SessionIndexCache {
    fn cache_path() -> Option<PathBuf> {
        let folder = if cfg!(debug_assertions) {
            "caipi-dev"
        } else {
            "caipi"
        };
        dirs::data_local_dir().map(|d| d.join(folder).join("session-index-cache.json"))
    }

    pub(crate) fn load() -> Self {
        let Some(path) = Self::cache_path() else {
            return Self::default();
        };
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub(crate) fn save(&self) {
        let Some(path) = Self::cache_path() else {
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let Ok(content) = serde_json::to_string(self) else {
            return;
        };
        if let Ok(mut tmp) = tempfile::NamedTempFile::new_in(path.parent().unwrap_or(Path::new(".")))
        {
            if tmp.write_all(content.as_bytes()).is_ok() {
                let _ = tmp.persist(&path);
            }
        }
    }

    pub(crate) fn get(&self, file_path: &str, mtime_secs: u64) -> Option<&SessionInfo> {
        self.entries
            .get(file_path)
            .filter(|(cached_mtime, _)| *cached_mtime == mtime_secs)
            .map(|(_, info)| info)
    }

    pub(crate) fn insert(&mut self, file_path: String, mtime_secs: u64, info: SessionInfo) {
        self.entries.insert(file_path, (mtime_secs, info));
    }

    pub(crate) fn cap_first_prompt_lengths(&mut self, max_len: usize) -> bool {
        let mut changed = false;
        for (_, (_, info)) in self.entries.iter_mut() {
            if info.first_prompt.chars().count() > max_len {
                info.first_prompt = info.first_prompt.chars().take(max_len).collect();
                changed = true;
            }
        }
        changed
    }
}

// ── Shared helpers ──────────────────────────────────────────────────────────

pub(crate) fn mtime_to_secs(mtime: std::time::SystemTime) -> u64 {
    mtime
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Get folder name from path (handles both Unix and Windows paths)
pub(crate) fn get_folder_name(path: &str) -> String {
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
pub(crate) fn encode_folder_path(path: &str) -> String {
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

/// Check if a filename stem is a valid UUID (8-4-4-4-12 hex pattern)
pub(crate) fn is_uuid_filename(stem: &str) -> bool {
    let parts: Vec<&str> = stem.split('-').collect();
    parts.len() == 5
        && is_hex_with_len(parts[0], 8)
        && is_hex_with_len(parts[1], 4)
        && is_hex_with_len(parts[2], 4)
        && is_hex_with_len(parts[3], 4)
        && is_hex_with_len(parts[4], 12)
}

pub(crate) fn is_hex_with_len(value: &str, len: usize) -> bool {
    value.len() == len && value.chars().all(|c| c.is_ascii_hexdigit())
}

pub(crate) fn trailing_uuid_like(stem: &str) -> Option<String> {
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

pub(crate) fn parse_rfc3339_timestamp(timestamp: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|dt| dt.timestamp())
        .unwrap_or(0)
}

/// Walk directory tree collecting .jsonl files with their modification times.
pub(crate) fn walk_jsonl_files_with_mtime(root: &Path) -> Vec<(PathBuf, std::time::SystemTime)> {
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

pub(crate) fn walk_jsonl_files(root: &Path) -> Vec<PathBuf> {
    walk_jsonl_files_with_mtime(root)
        .into_iter()
        .map(|(path, _)| path)
        .collect()
}

/// Convert history messages to chat messages (shared by both backends).
pub(crate) fn history_to_chat_messages(history: Vec<HistoryMessage>) -> Vec<ChatMessage> {
    history
        .into_iter()
        .map(|msg| ChatMessage {
            id: msg.id,
            role: msg.role,
            content: msg.content,
            timestamp: msg.timestamp,
        })
        .collect()
}

/// Group a flat list of sessions into project groups, preserving insertion order.
fn group_sessions_by_project(sessions: Vec<(SessionInfo, String)>) -> Vec<ProjectSessions> {
    let mut project_map: HashMap<String, ProjectSessions> = HashMap::new();
    let mut project_order: Vec<String> = Vec::new();

    for (session, folder_path) in sessions {
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

    project_order
        .into_iter()
        .filter_map(|path| project_map.remove(&path))
        .collect()
}

// ── Tauri commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_all_sessions(backend: Option<String>) -> Result<Vec<ProjectSessions>, String> {
    // Delegate to get_recent_sessions with a high limit for backwards compatibility
    get_recent_sessions(1000, backend).await
}

async fn get_recent_sessions_by_backend(
    limit: u32,
    backend: Option<String>,
) -> Result<Vec<ProjectSessions>, String> {
    if limit == 0 {
        return Ok(Vec::new());
    }

    if matches!(backend.as_deref(), Some("codex")) {
        let mut all_sessions = load_recent_codex_sessions(limit as usize)?;

        all_sessions.sort_by(|a, b| b.0.modified.cmp(&a.0.modified));
        all_sessions.truncate(limit as usize);

        return Ok(group_sessions_by_project(all_sessions));
    }

    let home_dir = dirs::home_dir().ok_or("Could not find home directory")?;
    let projects_dir = home_dir.join(".claude").join("projects");

    if !projects_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&projects_dir)
        .map_err(|e| format!("Failed to read projects directory: {}", e))?;

    // Phase 1: Collect all (path, mtime) pairs across all project dirs - metadata only, no parsing
    let mut all_files: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
    for entry in entries.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()) {
        let dir = entry.path();
        if let Ok(dir_entries) = fs::read_dir(&dir) {
            for file_entry in dir_entries.filter_map(|e| e.ok()) {
                let path = file_entry.path();
                if path.is_dir() {
                    continue;
                }
                if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                    continue;
                }
                let stem = match path.file_stem().and_then(|s| s.to_str()) {
                    Some(s) => s,
                    None => continue,
                };
                if !is_uuid_filename(stem) {
                    continue;
                }
                let mtime = file_entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                all_files.push((path, mtime));
            }
        }
    }

    // Phase 2: Sort by mtime descending
    all_files.sort_by(|a, b| b.1.cmp(&a.1));

    // Phase 3: Parse in recency order, using cache for unchanged files
    let mut cache = SessionIndexCache::load();
    let mut cache_dirty = false;
    let mut all_sessions: Vec<(SessionInfo, String)> = Vec::new();

    let limit_usize = limit as usize;
    // Keep parsing candidates until we gather up to the requested limit after validation.
    for (path, mtime) in all_files {
        let path_str = path.to_string_lossy().to_string();
        let mtime_s = mtime_to_secs(mtime);

        let session = if let Some(cached) = cache.get(&path_str, mtime_s) {
            cached.clone()
        } else if let Some(parsed) = parse_claude_session_summary_fast(&path, mtime) {
            cache.insert(path_str, mtime_s, parsed.clone());
            cache_dirty = true;
            parsed
        } else {
            continue;
        };

        let folder_path = session.folder_path.clone();
        if std::path::Path::new(&folder_path).exists() {
            all_sessions.push((session, folder_path));
            if limit_usize != 0 && all_sessions.len() >= limit_usize {
                break;
            }
        }
    }

    if cache_dirty {
        cache.save();
    }

    // Re-sort since parsed timestamps may differ slightly from mtime
    all_sessions.sort_by(|a, b| b.0.modified.cmp(&a.0.modified));

    Ok(group_sessions_by_project(all_sessions))
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

    let mut sessions: Vec<SessionInfo> = load_claude_session_index(&project_dir, None)
        .into_iter()
        .filter(|s| s.folder_path == folder_path)
        .collect();

    // Sort by modified date (most recent first)
    sessions.sort_by(|a, b| b.modified.cmp(&a.modified));

    Ok(sessions)
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
    fn test_is_uuid_filename() {
        assert!(is_uuid_filename("16703562-a9c7-4af0-8fd6-8f453295c8fc"));
        assert!(is_uuid_filename("dba2996f-69e1-4353-9f41-415af1d4232c"));
        assert!(!is_uuid_filename("not-a-uuid"));
        assert!(!is_uuid_filename("sessions-index"));
        assert!(!is_uuid_filename(""));
    }
}
