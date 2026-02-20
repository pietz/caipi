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
