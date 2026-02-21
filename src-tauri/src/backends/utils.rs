//! Shared utilities used by multiple backend adapters.

use std::sync::Arc;
use tokio::io::AsyncBufReadExt;
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;

use super::runtime::PermissionChannels;

/// Windows constant to hide console windows when spawning CLI subprocesses.
#[cfg(target_os = "windows")]
pub const CREATE_NO_WINDOW: u32 = 0x08000000;

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
            if !line.trim().is_empty() {
                log::debug!("[{label} stderr] {}", line.trim());
            }
        }
    })
}

/// Wait for a user permission decision via the shared permission channels.
///
/// Inserts a oneshot sender into `permission_channels` keyed by `permission_request_id`,
/// then races the receiver against a 60-second timeout and an abort notification.
///
/// Returns `true` if the user granted permission, `false` otherwise (denied, timeout, abort,
/// or channel cancellation).
pub async fn wait_for_permission(
    permission_channels: &PermissionChannels,
    permission_request_id: &str,
    abort_notify: &Arc<Notify>,
) -> bool {
    let (tx, rx) = tokio::sync::oneshot::channel();

    // Insert sender into channels map
    {
        let mut channels = permission_channels.lock().await;
        channels.insert(permission_request_id.to_string(), tx);
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
        channels.remove(permission_request_id);
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

    #[tokio::test]
    async fn wait_for_permission_returns_true_on_grant() {
        let channels = new_permission_channels();
        let abort = Arc::new(Notify::new());
        let req_id = "req-grant";

        let channels_clone = Arc::clone(&channels);
        let handle = tokio::spawn({
            let abort = Arc::clone(&abort);
            async move { wait_for_permission(&channels_clone, req_id, &abort).await }
        });

        // Wait for the sender to be inserted, then send a grant
        tokio::task::yield_now().await;
        let tx = {
            let mut map = channels.lock().await;
            map.remove(req_id).expect("sender should be registered")
        };
        tx.send(PermissionResponse { allowed: true }).ok();

        let result = handle.await.unwrap();
        assert!(result);

        // Channel entry should be cleaned up
        assert!(!channels.lock().await.contains_key(req_id));
    }

    #[tokio::test]
    async fn wait_for_permission_returns_false_on_deny() {
        let channels = new_permission_channels();
        let abort = Arc::new(Notify::new());
        let req_id = "req-deny";

        let channels_clone = Arc::clone(&channels);
        let handle = tokio::spawn({
            let abort = Arc::clone(&abort);
            async move { wait_for_permission(&channels_clone, req_id, &abort).await }
        });

        tokio::task::yield_now().await;
        let tx = {
            let mut map = channels.lock().await;
            map.remove(req_id).expect("sender should be registered")
        };
        tx.send(PermissionResponse { allowed: false }).ok();

        let result = handle.await.unwrap();
        assert!(!result);
        assert!(!channels.lock().await.contains_key(req_id));
    }

    #[tokio::test]
    async fn wait_for_permission_returns_false_on_abort() {
        let channels = new_permission_channels();
        let abort = Arc::new(Notify::new());
        let req_id = "req-abort";

        let channels_clone = Arc::clone(&channels);
        let handle = tokio::spawn({
            let abort = Arc::clone(&abort);
            async move { wait_for_permission(&channels_clone, req_id, &abort).await }
        });

        tokio::task::yield_now().await;
        // Verify sender was registered
        assert!(channels.lock().await.contains_key(req_id));

        // Notify abort
        abort.notify_waiters();

        let result = handle.await.unwrap();
        assert!(!result);
        assert!(!channels.lock().await.contains_key(req_id));
    }

    #[tokio::test]
    async fn wait_for_permission_returns_false_on_channel_drop() {
        let channels = new_permission_channels();
        let abort = Arc::new(Notify::new());
        let req_id = "req-drop";

        let channels_clone = Arc::clone(&channels);
        let handle = tokio::spawn({
            let abort = Arc::clone(&abort);
            async move { wait_for_permission(&channels_clone, req_id, &abort).await }
        });

        tokio::task::yield_now().await;
        // Drop the sender without sending a response
        {
            let mut map = channels.lock().await;
            let _dropped = map.remove(req_id);
            // tx is dropped here
        }

        let result = handle.await.unwrap();
        assert!(!result);
        assert!(!channels.lock().await.contains_key(req_id));
    }

    #[tokio::test(start_paused = true)]
    async fn wait_for_permission_returns_false_on_timeout() {
        // start_paused = true enables auto-advance of time in tokio test-util,
        // so the 60-second sleep completes instantly.
        let channels = new_permission_channels();
        let abort = Arc::new(Notify::new());
        let req_id = "req-timeout";

        let result = wait_for_permission(&channels, req_id, &abort).await;

        assert!(!result);
        assert!(!channels.lock().await.contains_key(req_id));
    }

    // ── spawn_stderr_drain ───────────────────────────────────────────────

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
