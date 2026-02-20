use serde_json::Value;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::backends::claude::tool_utils::extract_tool_target;
use crate::commands::chat::Message as ChatMessage;
use crate::commands::sessions::{
    encode_folder_path, get_folder_name, history_to_chat_messages, is_uuid_filename,
    parse_rfc3339_timestamp, HistoryMessage, HistoryTool, SessionInfo,
};

/// Fast summary: reads only the first ~20 lines of a Claude session .jsonl file
/// to extract session metadata and first prompt.
/// Uses filesystem mtime for the `modified` field.
pub(crate) fn parse_claude_session_summary_fast(
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

    // Truncate long prompts (char-safe to avoid panic on multi-byte UTF-8)
    if first_prompt.chars().count() > 200 {
        first_prompt = first_prompt.chars().take(200).collect();
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
pub(crate) fn load_claude_session_index(
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

pub(crate) fn resolve_session_file(
    folder_path: &str,
    session_id: &str,
) -> Result<Option<PathBuf>, String> {
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
pub(crate) fn load_session_history_messages(
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
        let timestamp = parse_rfc3339_timestamp(timestamp_str);

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
pub(crate) fn load_session_log_messages(
    folder_path: &str,
    session_id: &str,
) -> Result<Vec<ChatMessage>, String> {
    let history = load_session_history_messages(folder_path, session_id)?;
    Ok(history_to_chat_messages(history))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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
}
