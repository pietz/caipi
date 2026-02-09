use serde_json::Value;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub fn event_type(value: &Value) -> Option<&str> {
    value.get("type").and_then(Value::as_str)
}

pub fn first_string<'a>(value: &'a Value, paths: &[&[&str]]) -> Option<&'a str> {
    for path in paths {
        let mut current = value;
        let mut valid = true;
        for key in *path {
            match current.get(*key) {
                Some(next) => current = next,
                None => {
                    valid = false;
                    break;
                }
            }
        }
        if valid {
            if let Some(s) = current.as_str() {
                return Some(s);
            }
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenUsageTotals {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub total_tokens: u64,
}

impl TokenUsageTotals {
    pub fn saturating_sub(self, previous: Self) -> Self {
        Self {
            input_tokens: self.input_tokens.saturating_sub(previous.input_tokens),
            cached_input_tokens: self
                .cached_input_tokens
                .saturating_sub(previous.cached_input_tokens),
            output_tokens: self.output_tokens.saturating_sub(previous.output_tokens),
            reasoning_output_tokens: self
                .reasoning_output_tokens
                .saturating_sub(previous.reasoning_output_tokens),
            total_tokens: self.total_tokens.saturating_sub(previous.total_tokens),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenCountSnapshot {
    pub last_input_tokens: u64,
    pub model_context_window: u64,
    pub total_usage: TokenUsageTotals,
}

fn parse_token_usage_totals(value: &Value) -> Option<TokenUsageTotals> {
    Some(TokenUsageTotals {
        input_tokens: value.get("input_tokens")?.as_u64()?,
        cached_input_tokens: value
            .get("cached_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        output_tokens: value
            .get("output_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        reasoning_output_tokens: value
            .get("reasoning_output_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        total_tokens: value.get("total_tokens")?.as_u64()?,
    })
}

pub fn token_count_snapshot(value: &Value) -> Option<TokenCountSnapshot> {
    if value.get("type").and_then(Value::as_str) != Some("event_msg") {
        return None;
    }

    let payload = value.get("payload")?;
    if payload.get("type").and_then(Value::as_str) != Some("token_count") {
        return None;
    }

    let info = payload.get("info")?;
    if info.is_null() {
        return None;
    }

    let last_input_tokens = info
        .get("last_token_usage")
        .and_then(|v| v.get("input_tokens"))
        .and_then(Value::as_u64)?;
    let model_context_window = info.get("model_context_window").and_then(Value::as_u64)?;
    let total_usage = parse_token_usage_totals(info.get("total_token_usage")?)?;

    Some(TokenCountSnapshot {
        last_input_tokens,
        model_context_window,
        total_usage,
    })
}

fn walk_jsonl_files(root: &Path) -> Vec<PathBuf> {
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
                files.push(path);
            }
        }
    }
    files
}

pub fn find_rollout_path_for_thread(thread_id: &str) -> Option<PathBuf> {
    if thread_id.trim().is_empty() {
        return None;
    }

    let home_dir = dirs::home_dir()?;
    let sessions_root = home_dir.join(".codex").join("sessions");
    if !sessions_root.exists() {
        return None;
    }

    let mut newest_match: Option<(PathBuf, SystemTime)> = None;
    for path in walk_jsonl_files(&sessions_root) {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if !stem.contains(thread_id) {
            continue;
        }

        let modified = path
            .metadata()
            .and_then(|meta| meta.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);

        match &newest_match {
            Some((_, best)) if &modified <= best => {}
            _ => newest_match = Some((path, modified)),
        }
    }

    newest_match.map(|(path, _)| path)
}

pub fn latest_token_count_snapshot(
    path: &Path,
) -> Option<(TokenCountSnapshot, Option<TokenCountSnapshot>)> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut snapshots: Vec<TokenCountSnapshot> = Vec::new();

    for line in reader.lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let Some(snapshot) = token_count_snapshot(&value) else {
            continue;
        };

        let is_duplicate = snapshots
            .last()
            .map(|prev| prev.total_usage.total_tokens == snapshot.total_usage.total_tokens)
            .unwrap_or(false);
        if is_duplicate {
            continue;
        }

        snapshots.push(snapshot);
    }

    let latest = *snapshots.last()?;
    let previous = if snapshots.len() > 1 {
        Some(snapshots[snapshots.len() - 2])
    } else {
        None
    };

    Some((latest, previous))
}

pub fn token_usage(value: &Value) -> Option<u64> {
    let usage = value.get("usage")?;

    // Best-effort fallback when session rollout token_count snapshots are unavailable.
    if let Some(input) = usage.get("input_tokens").and_then(Value::as_u64) {
        if input > 0 {
            return Some(input);
        }
    }

    // Fallback: total_tokens (if the format changes)
    if let Some(n) = usage.get("total_tokens").and_then(Value::as_u64) {
        if n > 0 {
            return Some(n);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{latest_token_count_snapshot, token_count_snapshot, token_usage, TokenUsageTotals};
    use serde_json::json;
    use std::io::Write;

    #[test]
    fn token_count_snapshot_parses_valid_event_msg() {
        let value = json!({
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": {
                    "model_context_window": 258400,
                    "last_token_usage": {
                        "input_tokens": 10659
                    },
                    "total_token_usage": {
                        "input_tokens": 22486,
                        "cached_input_tokens": 11648,
                        "output_tokens": 612,
                        "reasoning_output_tokens": 248,
                        "total_tokens": 23098
                    }
                }
            }
        });

        let snapshot = token_count_snapshot(&value).expect("snapshot");
        assert_eq!(snapshot.last_input_tokens, 10_659);
        assert_eq!(snapshot.model_context_window, 258_400);
        assert_eq!(
            snapshot.total_usage,
            TokenUsageTotals {
                input_tokens: 22_486,
                cached_input_tokens: 11_648,
                output_tokens: 612,
                reasoning_output_tokens: 248,
                total_tokens: 23_098,
            }
        );
    }

    #[test]
    fn latest_token_count_snapshot_dedupes_by_total_tokens() {
        let mut temp = tempfile::NamedTempFile::new().expect("temp file");
        let first = json!({
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": {
                    "model_context_window": 258400,
                    "last_token_usage": {"input_tokens": 100},
                    "total_token_usage": {
                        "input_tokens": 1000,
                        "cached_input_tokens": 200,
                        "output_tokens": 50,
                        "reasoning_output_tokens": 0,
                        "total_tokens": 1050
                    }
                }
            }
        });
        let duplicate = json!({
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": {
                    "model_context_window": 258400,
                    "last_token_usage": {"input_tokens": 101},
                    "total_token_usage": {
                        "input_tokens": 1000,
                        "cached_input_tokens": 200,
                        "output_tokens": 50,
                        "reasoning_output_tokens": 0,
                        "total_tokens": 1050
                    }
                }
            }
        });
        let second = json!({
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": {
                    "model_context_window": 258400,
                    "last_token_usage": {"input_tokens": 300},
                    "total_token_usage": {
                        "input_tokens": 1400,
                        "cached_input_tokens": 200,
                        "output_tokens": 80,
                        "reasoning_output_tokens": 0,
                        "total_tokens": 1480
                    }
                }
            }
        });

        writeln!(temp, "{}", first).expect("write");
        writeln!(temp, "{}", duplicate).expect("write");
        writeln!(temp, "{}", second).expect("write");

        let (latest, previous) = latest_token_count_snapshot(temp.path()).expect("latest snapshot");
        assert_eq!(latest.last_input_tokens, 300);
        assert_eq!(latest.total_usage.total_tokens, 1480);
        assert_eq!(
            previous
                .expect("previous snapshot")
                .total_usage
                .total_tokens,
            1050
        );
    }

    #[test]
    fn token_usage_fallback_uses_input_tokens() {
        let value = json!({
            "usage": {
                "input_tokens": 1234,
                "cached_input_tokens": 1200,
                "output_tokens": 12
            }
        });
        assert_eq!(token_usage(&value), Some(1234));
    }
}
