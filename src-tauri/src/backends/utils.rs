//! Shared utilities used by multiple backend adapters.

use std::sync::Arc;
use tokio::io::AsyncBufReadExt;
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;

use super::runtime::PermissionChannels;

/// Windows constant to hide console windows when spawning CLI subprocesses.
#[cfg(target_os = "windows")]
pub const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Append Homebrew paths to the command's `PATH` environment variable on macOS.
///
/// Tauri apps don't inherit the user's shell PATH, so tools installed via
/// Homebrew (ffmpeg, node, etc.) are invisible to CLI subprocesses. This
/// prepends `/opt/homebrew/bin` and `/opt/homebrew/sbin` when they exist.
#[cfg(target_os = "macos")]
pub fn add_homebrew_paths(cmd: &mut tokio::process::Command) {
    use std::path::Path;

    let homebrew_bin = Path::new("/opt/homebrew/bin");
    if !homebrew_bin.exists() {
        return;
    }

    let current_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("/opt/homebrew/bin:/opt/homebrew/sbin:{current_path}");
    cmd.env("PATH", new_path);
}

/// Cancel and await a background task stored in an `Arc<Mutex<Option<JoinHandle<()>>>>`.
///
/// Takes the handle out of the mutex, aborts it, and awaits completion.
pub async fn abort_task_slot(slot: &Arc<Mutex<Option<JoinHandle<()>>>>) {
    let handle = {
        let mut guard = slot.lock().await;
        guard.take()
    };
    if let Some(handle) = handle {
        handle.abort();
        let _ = handle.await;
    }
}

const MAX_LOG_PREVIEW_CHARS: usize = 240;
const REDACTION_TOKEN: &str = "[REDACTED]";
const SECRET_KEYS: &[&str] = &[
    "api_key",
    "apikey",
    "access_token",
    "refresh_token",
    "token",
    "password",
    "secret",
];

/// Sanitized log preview for untrusted CLI text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizedLogText {
    pub text: String,
    pub original_len: usize,
    pub truncated: bool,
    pub redaction_count: usize,
}

/// Sanitize untrusted CLI text before logging.
///
/// Applies lightweight secret redaction and keeps only a short preview so we
/// never log full raw payloads from untrusted subprocess output.
pub fn sanitize_untrusted_log_text(input: &str) -> SanitizedLogText {
    let trimmed = input.trim();
    let original_len = trimmed.chars().count();

    if trimmed.is_empty() {
        return SanitizedLogText {
            text: String::new(),
            original_len,
            truncated: false,
            redaction_count: 0,
        };
    }

    let mut sanitized = trimmed.to_string();
    let mut redaction_count = 0;

    redaction_count += redact_bearer_tokens(&mut sanitized);
    for key in SECRET_KEYS {
        redaction_count += redact_secret_key_value(&mut sanitized, key);
    }

    let sanitized_len = sanitized.chars().count();
    let (text, truncated) = if sanitized_len > MAX_LOG_PREVIEW_CHARS {
        let omitted = sanitized_len - MAX_LOG_PREVIEW_CHARS;
        let preview: String = sanitized.chars().take(MAX_LOG_PREVIEW_CHARS).collect();
        (format!("{preview}... [truncated {omitted} chars]"), true)
    } else {
        (sanitized, false)
    };

    SanitizedLogText {
        text,
        original_len,
        truncated,
        redaction_count,
    }
}

fn redact_bearer_tokens(text: &mut String) -> usize {
    let mut redactions = 0;
    let mut search_from = 0;

    while let Some(idx) = find_ascii_case_insensitive(text, "bearer ", search_from) {
        let value_start = idx + "bearer ".len();
        let mut value_end = value_start;

        while value_end < text.len() {
            let byte = text.as_bytes()[value_end];
            if byte.is_ascii_whitespace() || matches!(byte, b',' | b';' | b'"' | b'\'') {
                break;
            }
            value_end += 1;
        }

        if value_end > value_start {
            if &text[value_start..value_end] != REDACTION_TOKEN {
                text.replace_range(value_start..value_end, REDACTION_TOKEN);
                redactions += 1;
            }
            search_from = value_start + REDACTION_TOKEN.len();
        } else {
            search_from = value_start;
        }
    }

    redactions
}

fn redact_secret_key_value(text: &mut String, key: &str) -> usize {
    let mut redactions = 0;
    let mut search_from = 0;

    while let Some(idx) = find_ascii_case_insensitive(text, key, search_from) {
        let key_end = idx + key.len();
        let bytes = text.as_bytes();

        if idx > 0 && !is_key_boundary(bytes[idx - 1]) {
            search_from = idx + 1;
            continue;
        }
        if key_end < bytes.len() && !is_key_boundary(bytes[key_end]) {
            search_from = idx + 1;
            continue;
        }

        let mut cursor = key_end;
        while cursor < text.len() && text.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= text.len() || !matches!(text.as_bytes()[cursor], b'=' | b':') {
            search_from = idx + 1;
            continue;
        }
        cursor += 1;
        while cursor < text.len() && text.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= text.len() {
            search_from = key_end;
            continue;
        }

        let quote = match text.as_bytes()[cursor] {
            b'"' => Some(b'"'),
            b'\'' => Some(b'\''),
            _ => None,
        };

        let value_start = if quote.is_some() { cursor + 1 } else { cursor };
        if value_start >= text.len() {
            search_from = key_end;
            continue;
        }

        let mut value_end = value_start;
        if let Some(quote_char) = quote {
            while value_end < text.len() {
                let byte = text.as_bytes()[value_end];
                if byte == quote_char || matches!(byte, b'\r' | b'\n') {
                    break;
                }
                value_end += 1;
            }
        } else {
            while value_end < text.len() {
                let byte = text.as_bytes()[value_end];
                if byte.is_ascii_whitespace() || matches!(byte, b',' | b';' | b'}' | b']' | b')')
                {
                    break;
                }
                value_end += 1;
            }
        }

        if value_end <= value_start {
            search_from = key_end;
            continue;
        }

        if &text[value_start..value_end] != REDACTION_TOKEN {
            text.replace_range(value_start..value_end, REDACTION_TOKEN);
            redactions += 1;
        }
        search_from = value_start + REDACTION_TOKEN.len();
    }

    redactions
}

fn is_key_boundary(byte: u8) -> bool {
    !(byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn find_ascii_case_insensitive(haystack: &str, needle: &str, from: usize) -> Option<usize> {
    if needle.is_empty() || from >= haystack.len() || needle.len() > haystack.len() {
        return None;
    }

    let haystack = haystack.as_bytes();
    let needle = needle.as_bytes();
    let last_start = haystack.len() - needle.len();

    for idx in from..=last_start {
        if haystack[idx..idx + needle.len()].eq_ignore_ascii_case(needle) {
            return Some(idx);
        }
    }
    None
}

/// Spawn a tokio task that drains stderr line-by-line, logging non-empty lines.
///
/// Prevents deadlock when the child process writes to stderr faster than we consume it.
pub fn spawn_stderr_drain(
    stderr: tokio::process::ChildStderr,
    label: &'static str,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut lines = tokio::io::BufReader::new(stderr).lines();
        while let Some(line) = lines.next_line().await.unwrap_or(None) {
            let sanitized = sanitize_untrusted_log_text(&line);
            if !sanitized.text.is_empty() {
                log::debug!(
                    "[{label} stderr] {} [len={}, redactions={}, truncated={}]",
                    sanitized.text,
                    sanitized.original_len,
                    sanitized.redaction_count,
                    sanitized.truncated
                );
            }
        }
    })
}

/// Wait for a user permission decision via the shared permission channels.
///
/// Inserts a oneshot sender into `permission_channels` keyed by
/// `(session_id, permission_request_id)`, then races the receiver against a
/// 60-second timeout and an abort notification.
///
/// Returns `true` if the user granted permission, `false` otherwise (denied, timeout, abort,
/// or channel cancellation).
pub async fn wait_for_permission(
    permission_channels: &PermissionChannels,
    session_id: &str,
    permission_request_id: &str,
    abort_notify: &Arc<Notify>,
) -> bool {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let key = (session_id.to_string(), permission_request_id.to_string());

    // Insert sender into channels map
    {
        let mut channels = permission_channels.lock().await;
        channels.insert(key.clone(), tx);
    }

    // Wait for user response, timeout, or abort
    let timeout = tokio::time::sleep(std::time::Duration::from_secs(60));
    tokio::pin!(timeout);
    tokio::pin!(rx);

    let allowed = tokio::select! {
        response = &mut rx => {
            response.map(|r| r.allowed).unwrap_or(false)
        }
        _ = &mut timeout => {
            false
        }
        _ = abort_notify.notified() => {
            false
        }
    };

    // Cleanup channel entry
    {
        let mut channels = permission_channels.lock().await;
        channels.remove(&key);
    }

    allowed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::runtime::PermissionResponse;
    use std::collections::HashMap;

    // ── abort_task_slot ──────────────────────────────────────────────────

    #[tokio::test]
    async fn abort_task_slot_cancels_running_task() {
        let slot: Arc<Mutex<Option<JoinHandle<()>>>> = Arc::new(Mutex::new(None));

        // Spawn a task that sleeps forever
        let handle = tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        });
        *slot.lock().await = Some(handle);

        abort_task_slot(&slot).await;

        // Slot should now be None
        assert!(slot.lock().await.is_none());
    }

    #[tokio::test]
    async fn abort_task_slot_noop_when_empty() {
        let slot: Arc<Mutex<Option<JoinHandle<()>>>> = Arc::new(Mutex::new(None));

        // Should not panic or hang
        abort_task_slot(&slot).await;

        assert!(slot.lock().await.is_none());
    }

    // ── wait_for_permission ──────────────────────────────────────────────

    fn new_permission_channels() -> PermissionChannels {
        Arc::new(Mutex::new(HashMap::new()))
    }

    fn permission_key(session_id: &str, req_id: &str) -> (String, String) {
        (session_id.to_string(), req_id.to_string())
    }

    #[tokio::test]
    async fn wait_for_permission_returns_true_on_grant() {
        let channels = new_permission_channels();
        let abort = Arc::new(Notify::new());
        let req_id = "req-grant";

        let channels_clone = Arc::clone(&channels);
        let handle = tokio::spawn({
            let abort = Arc::clone(&abort);
            async move { wait_for_permission(&channels_clone, "session-a", req_id, &abort).await }
        });

        // Wait for the sender to be inserted, then send a grant
        tokio::task::yield_now().await;
        let tx = {
            let mut map = channels.lock().await;
            map.remove(&permission_key("session-a", req_id))
                .expect("sender should be registered")
        };
        tx.send(PermissionResponse { allowed: true }).ok();

        let result = handle.await.unwrap();
        assert!(result);

        // Channel entry should be cleaned up
        assert!(!channels
            .lock()
            .await
            .contains_key(&permission_key("session-a", req_id)));
    }

    #[tokio::test]
    async fn wait_for_permission_returns_false_on_deny() {
        let channels = new_permission_channels();
        let abort = Arc::new(Notify::new());
        let req_id = "req-deny";

        let channels_clone = Arc::clone(&channels);
        let handle = tokio::spawn({
            let abort = Arc::clone(&abort);
            async move { wait_for_permission(&channels_clone, "session-a", req_id, &abort).await }
        });

        tokio::task::yield_now().await;
        let tx = {
            let mut map = channels.lock().await;
            map.remove(&permission_key("session-a", req_id))
                .expect("sender should be registered")
        };
        tx.send(PermissionResponse { allowed: false }).ok();

        let result = handle.await.unwrap();
        assert!(!result);
        assert!(!channels
            .lock()
            .await
            .contains_key(&permission_key("session-a", req_id)));
    }

    #[tokio::test]
    async fn wait_for_permission_returns_false_on_abort() {
        let channels = new_permission_channels();
        let abort = Arc::new(Notify::new());
        let req_id = "req-abort";

        let channels_clone = Arc::clone(&channels);
        let handle = tokio::spawn({
            let abort = Arc::clone(&abort);
            async move { wait_for_permission(&channels_clone, "session-a", req_id, &abort).await }
        });

        tokio::task::yield_now().await;
        // Verify sender was registered
        assert!(channels
            .lock()
            .await
            .contains_key(&permission_key("session-a", req_id)));

        // Notify abort
        abort.notify_waiters();

        let result = handle.await.unwrap();
        assert!(!result);
        assert!(!channels
            .lock()
            .await
            .contains_key(&permission_key("session-a", req_id)));
    }

    #[tokio::test]
    async fn wait_for_permission_returns_false_on_channel_drop() {
        let channels = new_permission_channels();
        let abort = Arc::new(Notify::new());
        let req_id = "req-drop";

        let channels_clone = Arc::clone(&channels);
        let handle = tokio::spawn({
            let abort = Arc::clone(&abort);
            async move { wait_for_permission(&channels_clone, "session-a", req_id, &abort).await }
        });

        tokio::task::yield_now().await;
        // Drop the sender without sending a response
        {
            let mut map = channels.lock().await;
            let _dropped = map.remove(&permission_key("session-a", req_id));
            // tx is dropped here
        }

        let result = handle.await.unwrap();
        assert!(!result);
        assert!(!channels
            .lock()
            .await
            .contains_key(&permission_key("session-a", req_id)));
    }

    #[tokio::test(start_paused = true)]
    async fn wait_for_permission_returns_false_on_timeout() {
        // start_paused = true enables auto-advance of time in tokio test-util,
        // so the 60-second sleep completes instantly.
        let channels = new_permission_channels();
        let abort = Arc::new(Notify::new());
        let req_id = "req-timeout";

        let result = wait_for_permission(&channels, "session-a", req_id, &abort).await;

        assert!(!result);
        assert!(!channels
            .lock()
            .await
            .contains_key(&permission_key("session-a", req_id)));
    }

    #[tokio::test]
    async fn wait_for_permission_scopes_same_request_id_by_session() {
        let channels = new_permission_channels();
        let abort = Arc::new(Notify::new());
        let req_id = "req-shared";

        let channels_a = Arc::clone(&channels);
        let abort_a = Arc::clone(&abort);
        let handle_a = tokio::spawn(async move {
            wait_for_permission(&channels_a, "session-a", req_id, &abort_a).await
        });

        let channels_b = Arc::clone(&channels);
        let abort_b = Arc::clone(&abort);
        let handle_b = tokio::spawn(async move {
            wait_for_permission(&channels_b, "session-b", req_id, &abort_b).await
        });

        tokio::task::yield_now().await;
        let (tx_a, tx_b) = {
            let mut map = channels.lock().await;
            let tx_a = map
                .remove(&permission_key("session-a", req_id))
                .expect("session-a sender should be registered");
            let tx_b = map
                .remove(&permission_key("session-b", req_id))
                .expect("session-b sender should be registered");
            (tx_a, tx_b)
        };

        tx_a.send(PermissionResponse { allowed: true }).ok();
        tx_b.send(PermissionResponse { allowed: false }).ok();

        assert!(handle_a.await.unwrap());
        assert!(!handle_b.await.unwrap());
    }

    // ── spawn_stderr_drain ───────────────────────────────────────────────

    #[test]
    fn sanitize_untrusted_log_text_redacts_common_secrets() {
        let input = "Authorization: Bearer abc.def.ghi token=xyz api_key:\"super-secret\" password = hunter2";

        let sanitized = sanitize_untrusted_log_text(input);

        assert!(sanitized.redaction_count >= 4);
        assert!(sanitized.text.contains("Bearer [REDACTED]"));
        assert!(!sanitized.text.contains("abc.def.ghi"));
        assert!(!sanitized.text.contains("token=xyz"));
        assert!(!sanitized.text.contains("super-secret"));
        assert!(!sanitized.text.contains("hunter2"));
        assert!(!sanitized.truncated);
    }

    #[test]
    fn sanitize_untrusted_log_text_truncates_long_input() {
        let input = format!("payload={}", "a".repeat(MAX_LOG_PREVIEW_CHARS + 64));

        let sanitized = sanitize_untrusted_log_text(&input);

        assert!(sanitized.truncated);
        assert!(sanitized.text.contains("[truncated"));
        assert_eq!(sanitized.original_len, input.chars().count());
        assert!(sanitized.text.chars().count() < input.chars().count());
    }

    #[tokio::test]
    async fn spawn_stderr_drain_completes_when_stderr_closes() {
        use tokio::process::Command;

        // Spawn a process that writes to stderr then exits
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("echo 'test line' >&2")
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("failed to spawn process");

        let stderr = child.stderr.take().expect("stderr should be captured");
        let handle = spawn_stderr_drain(stderr, "test");

        // The drain task should complete once the child exits and stderr closes
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;
        assert!(result.is_ok(), "stderr drain should complete promptly");

        // Clean up child
        let _ = child.wait().await;
    }
}
