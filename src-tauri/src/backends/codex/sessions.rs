use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::backends::codex::tool_utils::codex_tool_from_payload;
use crate::commands::chat::Message as ChatMessage;
use crate::commands::sessions::{
    get_folder_name, history_to_chat_messages, mtime_to_secs, parse_rfc3339_timestamp,
    trailing_uuid_like, walk_jsonl_files, walk_jsonl_files_with_mtime, HistoryMessage, HistoryTool,
    SessionIndexCache, SessionInfo,
};

pub(crate) fn codex_session_id_from_path(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    trailing_uuid_like(stem)
}

pub(crate) fn read_codex_session_meta(path: &Path) -> Option<(String, String)> {
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
pub(crate) fn codex_message_count(path: &Path) -> u32 {
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
pub(crate) fn parse_codex_session_summary_fast(
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
    if first_prompt.chars().count() > 200 {
        first_prompt = first_prompt.chars().take(200).collect();
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

pub(crate) fn load_codex_session_index(limit: Option<usize>) -> Result<Vec<SessionInfo>, String> {
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

pub(crate) fn load_recent_codex_sessions(
    limit: usize,
) -> Result<Vec<(SessionInfo, String)>, String> {
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
pub(crate) fn collect_existing_sessions_with_limit(
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

pub(crate) fn resolve_codex_session_file(
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

pub(crate) fn load_codex_history_messages(
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

pub(crate) fn load_codex_log_messages(
    session_id: &str,
    folder_path: Option<&str>,
) -> Result<Vec<ChatMessage>, String> {
    let history = load_codex_history_messages(session_id, folder_path)?;
    Ok(history_to_chat_messages(history))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::codex::tool_utils::codex_tool_from_payload;
    use crate::commands::sessions::get_folder_name;
    use tempfile::TempDir;

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
