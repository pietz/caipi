use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use crate::claude::tool_utils::extract_tool_target;
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

/// Session index cache: stores parsed session summaries keyed by (file_path, mtime_secs).
/// Avoids re-parsing unchanged files across app launches.
#[derive(Debug, Serialize, Deserialize, Default)]
struct SessionIndexCache {
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

    fn load() -> Self {
        let Some(path) = Self::cache_path() else {
            return Self::default();
        };
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
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

    fn get(&self, file_path: &str, mtime_secs: u64) -> Option<&SessionInfo> {
        self.entries
            .get(file_path)
            .filter(|(cached_mtime, _)| *cached_mtime == mtime_secs)
            .map(|(_, info)| info)
    }

    fn insert(&mut self, file_path: String, mtime_secs: u64, info: SessionInfo) {
        self.entries.insert(file_path, (mtime_secs, info));
    }

    fn cap_first_prompt_lengths(&mut self, max_len: usize) -> bool {
        let mut changed = false;
        for (_, (_, info)) in self.entries.iter_mut() {
            if info.first_prompt.len() > max_len {
                info.first_prompt.truncate(max_len);
                changed = true;
            }
        }
        changed
    }
}

fn mtime_to_secs(mtime: std::time::SystemTime) -> u64 {
    mtime
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
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

/// Check if a filename stem is a valid UUID (8-4-4-4-12 hex pattern)
fn is_uuid_filename(stem: &str) -> bool {
    let parts: Vec<&str> = stem.split('-').collect();
    parts.len() == 5
        && is_hex_with_len(parts[0], 8)
        && is_hex_with_len(parts[1], 4)
        && is_hex_with_len(parts[2], 4)
        && is_hex_with_len(parts[3], 4)
        && is_hex_with_len(parts[4], 12)
}

/// Fast summary: reads only the first ~20 lines of a Claude session .jsonl file
/// to extract session metadata and first prompt.
/// Uses filesystem mtime for the `modified` field.
fn parse_claude_session_summary_fast(
    path: &Path,
    mtime: std::time::SystemTime,
) -> Option<SessionInfo> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    let session_id = path.file_stem()?.to_str()?.to_string();
    let mut folder_path = String::new();
    let mut created = String::new();
    let mut first_prompt = String::new();
    let mut lines_read = 0u32;
    let mut has_real_message = false;

    for line in reader.lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        lines_read += 1;

        let json: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let event_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");

        // Skip non-message types for prompt extraction
        if event_type == "queue-operation" || event_type == "file-history-snapshot" {
            continue;
        }

        // Skip meta messages
        if json
            .get("isMeta")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            continue;
        }

        // Extract cwd from any user/assistant event
        if folder_path.is_empty() {
            if let Some(cwd) = json.get("cwd").and_then(|v| v.as_str()) {
                folder_path = cwd.to_string();
            }
        }

        // Extract timestamp
        if created.is_empty() {
            if let Some(ts) = json.get("timestamp").and_then(|v| v.as_str()) {
                if !ts.is_empty() {
                    created = ts.to_string();
                }
            }
        }

        // Track if we have at least one user+assistant pair
        if event_type == "user" || event_type == "assistant" {
            has_real_message = true;
        }

        // Extract first prompt from the first user message
        if event_type == "user" && first_prompt.is_empty() {
            if let Some(message) = json.get("message") {
                if let Some(content_str) = message.get("content").and_then(|v| v.as_str()) {
                    first_prompt = content_str.to_string();
                } else if let Some(content_arr) = message.get("content").and_then(|v| v.as_array())
                {
                    // Array content: extract text blocks
                    let texts: Vec<&str> = content_arr
                        .iter()
                        .filter_map(|block| {
                            if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                                block.get("text").and_then(|v| v.as_str())
                            } else {
                                None
                            }
                        })
                        .collect();
                    first_prompt = texts.join("\n");
                }
            }
        }

        // Once we have folder_path and first_prompt, stop
        if !folder_path.is_empty() && !first_prompt.is_empty() {
            break;
        }

        if lines_read >= 30 {
            break;
        }
    }

    if session_id.is_empty() || folder_path.is_empty() || !has_real_message {
        return None;
    }

    let folder_name = get_folder_name(&folder_path);
    if first_prompt.is_empty() {
        first_prompt = "No prompt".to_string();
    }

    // Truncate long prompts
    if first_prompt.len() > 200 {
        first_prompt = first_prompt[..200].to_string();
    }

    let modified = mtime
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .ok()
        .and_then(|d| chrono::DateTime::from_timestamp(d.as_secs() as i64, 0))
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default();

    Some(SessionInfo {
        session_id,
        folder_path,
        folder_name,
        first_prompt,
        message_count: 0, // Skip full-file scan for speed
        created: if created.is_empty() {
            modified.clone()
        } else {
            created
        },
        modified,
        backend: Some("claude".to_string()),
    })
}

/// Scan a Claude project directory for session .jsonl files and return session summaries.
fn load_claude_session_index(
    project_dir: &Path,
    limit: Option<usize>,
) -> Vec<SessionInfo> {
    if matches!(limit, Some(0)) {
        return Vec::new();
    }

    let entries = match fs::read_dir(project_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    // Collect UUID-named .jsonl files with their mtimes
    let mut files: Vec<(PathBuf, std::time::SystemTime)> = entries
        .filter_map(|e| e.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                return None;
            }
            if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                return None;
            }
            let stem = path.file_stem()?.to_str()?;
            if !is_uuid_filename(stem) {
                return None;
            }
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            Some((path, mtime))
        })
        .collect();

    // Sort by mtime descending (most recent first)
    files.sort_by(|a, b| b.1.cmp(&a.1));

    let mut sessions: Vec<SessionInfo> = Vec::new();
    for (path, mtime) in files {
        if let Some(session) = parse_claude_session_summary_fast(&path, mtime) {
            sessions.push(session);
            if let Some(n) = limit {
                if sessions.len() >= n {
                    break;
                }
            }
        }
    }

    sessions
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

#[cfg(test)]
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
    if first_prompt.len() > 200 {
        first_prompt.truncate(200);
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
        message_count: 0, // Skip full-file scan for speed
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
    if matches!(limit, Some(0)) {
        return Ok(Vec::new());
    }

    let home_dir = dirs::home_dir().ok_or("Could not find home directory")?;
    let sessions_root = home_dir.join(".codex").join("sessions");
    if !sessions_root.exists() {
        return Ok(Vec::new());
    }

    let mut files = walk_jsonl_files_with_mtime(&sessions_root);

    // Sort by mtime descending (most recent first)
    files.sort_by(|a, b| b.1.cmp(&a.1));

    let mut cache = SessionIndexCache::load();
    let mut cache_dirty = cache.cap_first_prompt_lengths(200);
    let mut sessions: Vec<SessionInfo> = Vec::new();

    for (path, mtime) in files {
        let path_str = path.to_string_lossy().to_string();
        let mtime_s = mtime_to_secs(mtime);

        let session = if let Some(cached) = cache.get(&path_str, mtime_s) {
            cached.clone()
        } else if let Some(parsed) = parse_codex_session_summary_fast(&path, mtime) {
            cache.insert(path_str, mtime_s, parsed.clone());
            cache_dirty = true;
            parsed
        } else {
            continue;
        };
        sessions.push(session);
        // Stop once we have enough valid sessions so limit reflects parsed output.
        if let Some(n) = limit {
            if sessions.len() >= n {
                break;
            }
        }
    }

    if cache_dirty {
        cache.save();
    }

    sessions.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(sessions)
}

fn load_recent_codex_sessions(limit: usize) -> Result<Vec<(SessionInfo, String)>, String> {
    if limit == 0 {
        return Ok(Vec::new());
    }

    let home_dir = dirs::home_dir().ok_or("Could not find home directory")?;
    let sessions_root = home_dir.join(".codex").join("sessions");
    if !sessions_root.exists() {
        return Ok(Vec::new());
    }

    let mut files = walk_jsonl_files_with_mtime(&sessions_root);
    files.sort_by(|a, b| b.1.cmp(&a.1));

    let mut cache = SessionIndexCache::load();
    let mut cache_dirty = cache.cap_first_prompt_lengths(200);
    let mut collected: Vec<(SessionInfo, String)> = Vec::new();

    for (path, mtime) in files {
        let path_str = path.to_string_lossy().to_string();
        let mtime_s = mtime_to_secs(mtime);

        let session = if let Some(cached) = cache.get(&path_str, mtime_s) {
            cached.clone()
        } else if let Some(parsed) = parse_codex_session_summary_fast(&path, mtime) {
            cache.insert(path_str, mtime_s, parsed.clone());
            cache_dirty = true;
            parsed
        } else {
            continue;
        };

        let folder_path = session.folder_path.clone();
        if !std::path::Path::new(&folder_path).exists() {
            continue;
        }

        collected.push((session, folder_path));
        if collected.len() >= limit {
            break;
        }
    }

    if cache_dirty {
        cache.save();
    }

    collected.sort_by(|a, b| b.0.modified.cmp(&a.0.modified));
    Ok(collected)
}

#[cfg(test)]
fn collect_existing_sessions_with_limit(
    sessions: impl IntoIterator<Item = SessionInfo>,
    limit: usize,
) -> Vec<(SessionInfo, String)> {
    let mut collected: Vec<(SessionInfo, String)> = Vec::new();
    for session in sessions {
        let folder_path = session.folder_path.clone();
        if !std::path::Path::new(&folder_path).exists() {
            continue;
        }
        collected.push((session, folder_path));
        if limit != 0 && collected.len() >= limit {
            break;
        }
    }
    collected
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
fn parse_function_arguments(payload: &Value) -> Option<Value> {
    let arguments = payload.get("arguments")?;
    if arguments.is_object() || arguments.is_array() {
        return Some(arguments.clone());
    }
    arguments
        .as_str()
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
}

fn first_array_entry<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.get(key)?.as_array()?.first()
}

fn web_run_target_from_args(args: &Value) -> String {
    if let Some(query) = args
        .get("search_query")
        .and_then(Value::as_array)
        .and_then(|entries| entries.iter().find_map(|entry| entry.get("q")))
        .and_then(Value::as_str)
    {
        return query.to_string();
    }

    if let Some(query) = args
        .get("image_query")
        .and_then(Value::as_array)
        .and_then(|entries| entries.iter().find_map(|entry| entry.get("q")))
        .and_then(Value::as_str)
    {
        return query.to_string();
    }

    if let Some(reference) = first_array_entry(args, "open")
        .and_then(|entry| entry.get("ref_id"))
        .and_then(Value::as_str)
    {
        return reference.to_string();
    }

    if let Some(pattern) = first_array_entry(args, "find")
        .and_then(|entry| entry.get("pattern"))
        .and_then(Value::as_str)
    {
        return pattern.to_string();
    }

    if let Some(location) = first_array_entry(args, "weather")
        .and_then(|entry| entry.get("location"))
        .and_then(Value::as_str)
    {
        return location.to_string();
    }

    if let Some(ticker) = first_array_entry(args, "finance")
        .and_then(|entry| entry.get("ticker"))
        .and_then(Value::as_str)
    {
        return ticker.to_string();
    }

    if let Some(offset) = first_array_entry(args, "time")
        .and_then(|entry| entry.get("utc_offset"))
        .and_then(Value::as_str)
    {
        return offset.to_string();
    }

    if let Some(reference) = first_array_entry(args, "click")
        .and_then(|entry| entry.get("ref_id"))
        .and_then(Value::as_str)
    {
        return reference.to_string();
    }

    "web.run".to_string()
}

fn codex_tool_from_payload(payload: &Value) -> Option<(String, String)> {
    let payload_type = payload.get("type").and_then(|v| v.as_str())?;
    match payload_type {
        "function_call" => {
            let name = payload
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("command_execution");
            let args = parse_function_arguments(payload);

            if name == "web.run" {
                let has_search_queries = args
                    .as_ref()
                    .map(|value| {
                        value
                            .get("search_query")
                            .or_else(|| value.get("image_query"))
                            .is_some()
                    })
                    .unwrap_or(false);
                let tool_type = if has_search_queries {
                    "web_search"
                } else {
                    "web_fetch"
                };
                let target = args
                    .as_ref()
                    .map(web_run_target_from_args)
                    .unwrap_or_else(|| "web.run".to_string());
                return Some((tool_type.to_string(), target));
            }

            let tool_type = match name {
                "exec_command" => "command_execution",
                _ => name,
            };
            let target = args
                .as_ref()
                .and_then(|value| {
                    value
                        .get("cmd")
                        .or_else(|| value.get("query"))
                        .or_else(|| value.get("command"))
                        .or_else(|| value.get("task"))
                        .or_else(|| value.get("prompt"))
                        .or_else(|| value.get("description"))
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
    if limit == 0 {
        return Ok(Vec::new());
    }

    if matches!(backend.as_deref(), Some("codex")) {
        let mut all_sessions = load_recent_codex_sessions(limit as usize)?;

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

    let mut sessions: Vec<SessionInfo> = load_claude_session_index(&project_dir, None)
        .into_iter()
        .filter(|s| s.folder_path == folder_path)
        .collect();

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
    fn test_is_uuid_filename() {
        assert!(is_uuid_filename("16703562-a9c7-4af0-8fd6-8f453295c8fc"));
        assert!(is_uuid_filename("dba2996f-69e1-4353-9f41-415af1d4232c"));
        assert!(!is_uuid_filename("not-a-uuid"));
        assert!(!is_uuid_filename("sessions-index"));
        assert!(!is_uuid_filename(""));
    }

    #[test]
    fn test_parse_claude_session_summary_fast_basic() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir
            .path()
            .join("dba2996f-69e1-4353-9f41-415af1d4232c.jsonl");
        let content = r#"{"type":"user","cwd":"/Users/test/project","sessionId":"dba2996f-69e1-4353-9f41-415af1d4232c","message":{"role":"user","content":"hello world"},"uuid":"abc","timestamp":"2026-02-03T16:25:55.830Z"}
{"type":"assistant","cwd":"/Users/test/project","sessionId":"dba2996f-69e1-4353-9f41-415af1d4232c","message":{"role":"assistant","content":[{"type":"text","text":"hi there"}]},"uuid":"def","timestamp":"2026-02-03T16:26:00.000Z"}"#;
        std::fs::write(&path, content).unwrap();

        let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
        let result = parse_claude_session_summary_fast(&path, mtime);
        assert!(result.is_some());
        let session = result.unwrap();
        assert_eq!(session.session_id, "dba2996f-69e1-4353-9f41-415af1d4232c");
        assert_eq!(session.folder_path, "/Users/test/project");
        assert_eq!(session.first_prompt, "hello world");
        assert_eq!(session.backend.as_deref(), Some("claude"));
    }

    #[test]
    fn test_parse_claude_session_summary_fast_skips_queue_operations() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir
            .path()
            .join("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee.jsonl");
        let content = r#"{"type":"queue-operation","operation":"dequeue","timestamp":"2026-02-03T16:25:55.825Z","sessionId":"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"}
{"type":"user","cwd":"/tmp/test","sessionId":"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee","message":{"role":"user","content":"test prompt"},"uuid":"abc","timestamp":"2026-02-03T16:25:56.000Z"}
{"type":"assistant","cwd":"/tmp/test","sessionId":"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee","message":{"role":"assistant","content":[{"type":"text","text":"response"}]},"uuid":"def","timestamp":"2026-02-03T16:26:00.000Z"}"#;
        std::fs::write(&path, content).unwrap();

        let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
        let result = parse_claude_session_summary_fast(&path, mtime);
        assert!(result.is_some());
        let session = result.unwrap();
        assert_eq!(session.first_prompt, "test prompt");
        assert_eq!(session.folder_path, "/tmp/test");
    }

    #[test]
    fn test_parse_claude_session_summary_fast_skips_meta_messages() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir
            .path()
            .join("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee.jsonl");
        let content = r#"{"type":"user","isMeta":true,"cwd":"/tmp/test","sessionId":"x","message":{"role":"user","content":"meta stuff"},"uuid":"a","timestamp":"2026-01-01T00:00:00Z"}
{"type":"user","cwd":"/tmp/test","sessionId":"x","message":{"role":"user","content":"real prompt"},"uuid":"b","timestamp":"2026-01-01T00:00:01Z"}
{"type":"assistant","cwd":"/tmp/test","sessionId":"x","message":{"role":"assistant","content":[{"type":"text","text":"ok"}]},"uuid":"c","timestamp":"2026-01-01T00:00:02Z"}"#;
        std::fs::write(&path, content).unwrap();

        let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
        let result = parse_claude_session_summary_fast(&path, mtime);
        assert!(result.is_some());
        let session = result.unwrap();
        assert_eq!(session.first_prompt, "real prompt");
    }

    #[test]
    fn test_parse_claude_session_summary_fast_no_messages() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir
            .path()
            .join("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee.jsonl");
        let content = r#"{"type":"queue-operation","operation":"dequeue","timestamp":"2026-02-03T16:25:55.825Z"}"#;
        std::fs::write(&path, content).unwrap();

        let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert!(parse_claude_session_summary_fast(&path, mtime).is_none());
    }

    #[test]
    fn test_load_claude_session_index_filters_uuid_files() {
        let temp_dir = TempDir::new().unwrap();

        // Valid session file
        let session_content = r#"{"type":"user","cwd":"/tmp/proj","sessionId":"x","message":{"role":"user","content":"hi"},"uuid":"a","timestamp":"2026-01-01T00:00:00Z"}
{"type":"assistant","cwd":"/tmp/proj","sessionId":"x","message":{"role":"assistant","content":[{"type":"text","text":"hello"}]},"uuid":"b","timestamp":"2026-01-01T00:00:01Z"}"#;
        std::fs::write(
            temp_dir
                .path()
                .join("dba2996f-69e1-4353-9f41-415af1d4232c.jsonl"),
            session_content,
        )
        .unwrap();

        // Non-UUID file (should be skipped)
        std::fs::write(
            temp_dir.path().join("sessions-index.json"),
            "{}",
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("not-a-session.jsonl"),
            session_content,
        )
        .unwrap();

        let sessions = load_claude_session_index(temp_dir.path(), None);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "dba2996f-69e1-4353-9f41-415af1d4232c");
    }

    #[test]
    fn test_load_claude_session_index_respects_limit() {
        let temp_dir = TempDir::new().unwrap();
        let content = r#"{"type":"user","cwd":"/tmp/proj","sessionId":"x","message":{"role":"user","content":"hi"},"uuid":"a","timestamp":"2026-01-01T00:00:00Z"}
{"type":"assistant","cwd":"/tmp/proj","sessionId":"x","message":{"role":"assistant","content":[{"type":"text","text":"hello"}]},"uuid":"b","timestamp":"2026-01-01T00:00:01Z"}"#;

        // Create 3 session files
        for uuid in [
            "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
            "bbbbbbbb-cccc-dddd-eeee-ffffffffffff",
            "cccccccc-dddd-eeee-ffff-aaaaaaaaaaaa",
        ] {
            std::fs::write(
                temp_dir.path().join(format!("{}.jsonl", uuid)),
                content,
            )
            .unwrap();
        }

        let sessions = load_claude_session_index(temp_dir.path(), Some(2));
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn test_load_claude_session_index_fills_limit_after_invalid_recent_file() {
        let temp_dir = TempDir::new().unwrap();
        let valid_content = r#"{"type":"user","cwd":"/tmp/proj","sessionId":"x","message":{"role":"user","content":"hi"},"uuid":"a","timestamp":"2026-01-01T00:00:00Z"}
{"type":"assistant","cwd":"/tmp/proj","sessionId":"x","message":{"role":"assistant","content":[{"type":"text","text":"hello"}]},"uuid":"b","timestamp":"2026-01-01T00:00:01Z"}"#;
        let invalid_content = r#"{"type":"file-history-snapshot","isSnapshot":true}"#;

        let valid_old = temp_dir
            .path()
            .join("11111111-1111-4111-8111-111111111111.jsonl");
        let valid_new = temp_dir
            .path()
            .join("22222222-2222-4222-8222-222222222222.jsonl");
        let invalid_newest = temp_dir
            .path()
            .join("33333333-3333-4333-8333-333333333333.jsonl");

        std::fs::write(&valid_old, valid_content).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&valid_new, valid_content).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&invalid_newest, invalid_content).unwrap();

        let sessions = load_claude_session_index(temp_dir.path(), Some(2));
        assert_eq!(sessions.len(), 2);
        assert!(
            sessions
                .iter()
                .all(|session| session.session_id != "33333333-3333-4333-8333-333333333333")
        );
    }

    #[test]
    fn test_collect_existing_sessions_with_limit_skips_missing_folders() {
        let temp_dir = TempDir::new().unwrap();
        let existing_a = temp_dir.path().join("a");
        let existing_b = temp_dir.path().join("b");
        std::fs::create_dir_all(&existing_a).unwrap();
        std::fs::create_dir_all(&existing_b).unwrap();

        let make_session = |session_id: &str, folder_path: String| SessionInfo {
            session_id: session_id.to_string(),
            folder_path: folder_path.clone(),
            folder_name: get_folder_name(&folder_path),
            first_prompt: "prompt".to_string(),
            message_count: 0,
            created: "2026-01-01T00:00:00Z".to_string(),
            modified: "2026-01-01T00:00:00Z".to_string(),
            backend: Some("codex".to_string()),
        };

        let sessions = vec![
            make_session("missing", "/tmp/does-not-exist-caipi-test".to_string()),
            make_session("existing-a", existing_a.to_string_lossy().to_string()),
            make_session("existing-b", existing_b.to_string_lossy().to_string()),
        ];

        let collected = collect_existing_sessions_with_limit(sessions, 2);
        assert_eq!(collected.len(), 2);
        assert_eq!(collected[0].0.session_id, "existing-a");
        assert_eq!(collected[1].0.session_id, "existing-b");
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

    #[test]
    fn test_codex_tool_from_payload_function_call() {
        let payload = serde_json::json!({
            "type": "function_call",
            "name": "exec_command",
            "arguments": "{\"cmd\":\"ls -la\"}"
        });
        let result = codex_tool_from_payload(&payload);
        assert!(result.is_some());
        let (tool_type, target) = result.unwrap();
        assert_eq!(tool_type, "command_execution");
        assert_eq!(target, "ls -la");
    }

    #[test]
    fn test_codex_tool_from_payload_web_search() {
        let payload = serde_json::json!({
            "type": "web_search_call",
            "action": {
                "query": "rust async"
            }
        });
        let result = codex_tool_from_payload(&payload);
        assert!(result.is_some());
        let (tool_type, target) = result.unwrap();
        assert_eq!(tool_type, "web_search");
        assert_eq!(target, "rust async");
    }

    #[test]
    fn test_codex_tool_from_payload_unknown_type() {
        let payload = serde_json::json!({
            "type": "text",
            "content": "hello"
        });
        assert!(codex_tool_from_payload(&payload).is_none());
    }

    #[test]
    fn test_codex_tool_from_payload_custom_function_name() {
        let payload = serde_json::json!({
            "type": "function_call",
            "name": "search_files",
            "arguments": "{\"query\":\"TODO\"}"
        });
        let result = codex_tool_from_payload(&payload);
        assert!(result.is_some());
        let (tool_type, target) = result.unwrap();
        assert_eq!(tool_type, "search_files");
        assert_eq!(target, "TODO");
    }

    #[test]
    fn test_codex_message_count_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("empty.jsonl");
        std::fs::write(&path, "").unwrap();
        assert_eq!(codex_message_count(&path), 0);
    }

    #[test]
    fn test_codex_message_count_malformed_lines() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("malformed.jsonl");
        let content = "not json\n{\"type\":\"event_msg\",\"payload\":{\"type\":\"user_message\",\"message\":\"hi\"}}\n\n{broken json\n";
        std::fs::write(&path, content).unwrap();
        assert_eq!(codex_message_count(&path), 1);
    }

    #[test]
    fn test_codex_session_summary_fast_basic() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("session-019a4b60-a492-7f12-9abe-73797723f5b1.jsonl");
        let content = r#"{"type":"session_meta","payload":{"id":"019a4b60-a492-7f12-9abe-73797723f5b1","cwd":"/tmp/project"},"timestamp":"2026-01-01T00:00:00Z"}
{"type":"event_msg","payload":{"type":"user_message","message":"hello world"},"timestamp":"2026-01-01T00:00:01Z"}
{"type":"event_msg","payload":{"type":"agent_message","message":"hi"},"timestamp":"2026-01-01T00:00:02Z"}
{"type":"event_msg","payload":{"type":"user_message","message":"bye"},"timestamp":"2026-01-01T00:00:03Z"}
{"type":"event_msg","payload":{"type":"agent_message","message":"goodbye"},"timestamp":"2026-01-01T00:00:04Z"}"#;
        std::fs::write(&path, content).unwrap();

        let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
        let result = parse_codex_session_summary_fast(&path, mtime);
        assert!(result.is_some());
        let session = result.unwrap();
        assert_eq!(session.session_id, "019a4b60-a492-7f12-9abe-73797723f5b1");
        assert_eq!(session.folder_path, "/tmp/project");
        assert_eq!(session.first_prompt, "hello world");
        assert_eq!(session.message_count, 0); // Skipped for speed in index view
        assert_eq!(session.backend.as_deref(), Some("codex"));
    }

    #[test]
    fn test_codex_session_summary_fast_missing_meta() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("no-meta.jsonl");
        let content = r#"{"type":"event_msg","payload":{"type":"user_message","message":"hello"},"timestamp":"2026-01-01T00:00:00Z"}"#;
        std::fs::write(&path, content).unwrap();

        let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
        // No session_meta means no folder_path => should return None
        let result = parse_codex_session_summary_fast(&path, mtime);
        assert!(result.is_none());
    }

    #[test]
    fn test_codex_tool_from_payload_web_search_url() {
        let payload = serde_json::json!({
            "type": "web_search_call",
            "action": {
                "url": "https://example.com"
            }
        });
        let result = codex_tool_from_payload(&payload);
        assert!(result.is_some());
        let (tool_type, target) = result.unwrap();
        assert_eq!(tool_type, "web_search");
        assert_eq!(target, "https://example.com");
    }

    #[test]
    fn test_codex_tool_from_payload_function_call_command_key() {
        let payload = serde_json::json!({
            "type": "function_call",
            "name": "exec_command",
            "arguments": "{\"command\":\"npm install\"}"
        });
        let result = codex_tool_from_payload(&payload);
        assert!(result.is_some());
        let (_, target) = result.unwrap();
        assert_eq!(target, "npm install");
    }

    #[test]
    fn test_codex_tool_from_payload_web_run_search_maps_to_web_search() {
        let payload = serde_json::json!({
            "type": "function_call",
            "name": "web.run",
            "arguments": "{\"search_query\":[{\"q\":\"rust async runtime\"}]}"
        });
        let result = codex_tool_from_payload(&payload);
        assert!(result.is_some());
        let (tool_type, target) = result.unwrap();
        assert_eq!(tool_type, "web_search");
        assert_eq!(target, "rust async runtime");
    }

    #[test]
    fn test_codex_tool_from_payload_web_run_open_maps_to_web_fetch() {
        let payload = serde_json::json!({
            "type": "function_call",
            "name": "web.run",
            "arguments": "{\"open\":[{\"ref_id\":\"turn0search0\"}]}"
        });
        let result = codex_tool_from_payload(&payload);
        assert!(result.is_some());
        let (tool_type, target) = result.unwrap();
        assert_eq!(tool_type, "web_fetch");
        assert_eq!(target, "turn0search0");
    }

    #[test]
    fn test_read_codex_session_meta_valid() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("session.jsonl");
        let content = r#"{"type":"session_meta","payload":{"id":"abc-123","cwd":"/home/user/project"},"timestamp":"2026-01-01T00:00:00Z"}"#;
        std::fs::write(&path, content).unwrap();

        let result = read_codex_session_meta(&path);
        assert!(result.is_some());
        let (id, cwd) = result.unwrap();
        assert_eq!(id, "abc-123");
        assert_eq!(cwd, "/home/user/project");
    }

    #[test]
    fn test_read_codex_session_meta_no_meta_line() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("no-meta.jsonl");
        let content = r#"{"type":"event_msg","payload":{"type":"user_message","message":"hi"},"timestamp":"2026-01-01T00:00:00Z"}"#;
        std::fs::write(&path, content).unwrap();

        assert!(read_codex_session_meta(&path).is_none());
    }
}
