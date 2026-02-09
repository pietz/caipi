use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use crate::backends::emit_chat_event;
use crate::backends::session::BackendSession;
use crate::backends::types::{
    AuthStatus, Backend, BackendCapabilities, BackendError, BackendKind, InstallStatus, ModelInfo,
    PermissionModel, SessionConfig,
};
use crate::commands::chat::{ChatEvent, Message};
use crate::commands::sessions::load_codex_log_messages;
use crate::commands::setup::{
    check_backend_cli_authenticated_internal, check_backend_cli_installed_internal,
};

use super::cli_protocol::{
    event_type, find_rollout_path_for_thread, first_string, latest_token_count_snapshot,
    token_usage,
};

fn codex_permission_args_for_mode(mode: &str) -> Vec<String> {
    match mode {
        // Match "Allow All" semantics for Codex.
        "bypassPermissions" => vec!["--dangerously-bypass-approvals-and-sandbox".to_string()],
        // "Edit": allow workspace writes but avoid approval prompts in non-interactive exec mode.
        "acceptEdits" => vec![
            "--sandbox".to_string(),
            "workspace-write".to_string(),
            "-c".to_string(),
            "approval_policy=\"never\"".to_string(),
        ],
        // "Default" (and unknown): keep Codex constrained to read-only with no prompts.
        _ => vec![
            "--sandbox".to_string(),
            "read-only".to_string(),
            "-c".to_string(),
            "approval_policy=\"never\"".to_string(),
        ],
    }
}

fn clean_thinking_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.len() >= 4 && trimmed.starts_with("**") && trimmed.ends_with("**") {
        trimmed[2..trimmed.len() - 2].trim().to_string()
    } else {
        trimmed.to_string()
    }
}

fn final_tool_status(tool_type: &str, item_status: &str, exit_code: Option<i64>) -> &'static str {
    if item_status != "completed" {
        return "error";
    }

    // For shell commands, require an explicit successful exit code.
    // This avoids showing "completed" when execution was blocked/denied by sandbox.
    if tool_type == "command_execution" {
        if exit_code == Some(0) {
            "completed"
        } else {
            "error"
        }
    } else {
        "completed"
    }
}

/// Codex CLI backend implementation.
pub struct CodexBackend;

impl CodexBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CodexBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Backend for CodexBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Codex
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            permission_model: PermissionModel::SessionLevel,
            supports_streaming: true,
            supports_abort: true,
            supports_resume: true,
            supports_extended_thinking: true,
            available_models: vec![
                ModelInfo {
                    id: "gpt-5.3-codex".to_string(),
                    name: "GPT-5.3 Codex".to_string(),
                    supports_thinking: true,
                },
                ModelInfo {
                    id: "gpt-5.2".to_string(),
                    name: "GPT-5.2".to_string(),
                    supports_thinking: true,
                },
                ModelInfo {
                    id: "gpt-5.1-codex-mini".to_string(),
                    name: "GPT-5.1 Codex Mini".to_string(),
                    supports_thinking: false,
                },
            ],
        }
    }

    async fn check_installed(&self) -> Result<InstallStatus, BackendError> {
        let status = check_backend_cli_installed_internal("codex").await;
        Ok(InstallStatus {
            installed: status.installed,
            version: status.version,
            path: status.path,
        })
    }

    async fn check_authenticated(&self) -> Result<AuthStatus, BackendError> {
        let status = check_backend_cli_authenticated_internal("codex").await;
        Ok(AuthStatus {
            authenticated: status.authenticated,
        })
    }

    async fn create_session(
        &self,
        config: SessionConfig,
        app_handle: AppHandle,
    ) -> Result<Arc<dyn BackendSession>, BackendError> {
        Ok(Arc::new(CodexSession::new(config, app_handle).await?))
    }
}

pub struct CodexSession {
    id: String,
    folder_path: String,
    permission_mode: Arc<RwLock<String>>,
    model: Arc<RwLock<String>>,
    thinking_level: Arc<RwLock<String>>,
    cli_path: Option<String>,
    app_handle: AppHandle,
    thread_id: Arc<RwLock<Option<String>>>,
    messages: Arc<RwLock<Vec<Message>>>,
    active_process: Arc<Mutex<Option<Child>>>,
    in_flight: Arc<AtomicBool>,
    current_turn_id: Arc<RwLock<Option<String>>>,
    aborted_turn_id: Arc<RwLock<Option<String>>>,
}

impl CodexSession {
    async fn release_turn_state(
        turn_id: &str,
        active_process: &Arc<Mutex<Option<Child>>>,
        in_flight: &Arc<AtomicBool>,
        current_turn_id: &Arc<RwLock<Option<String>>>,
    ) {
        *active_process.lock().await = None;

        if current_turn_id.read().await.as_deref() == Some(turn_id) {
            *current_turn_id.write().await = None;
        }

        in_flight.store(false, Ordering::SeqCst);
    }

    async fn new(config: SessionConfig, app_handle: AppHandle) -> Result<Self, BackendError> {
        let folder_path = config.folder_path.clone();
        let resume_session_id = config.resume_session_id.clone();
        let initial_messages = if let Some(session_id) = resume_session_id.as_deref() {
            load_codex_log_messages(session_id, Some(folder_path.as_str())).unwrap_or_default()
        } else {
            Vec::new()
        };

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            folder_path,
            permission_mode: Arc::new(RwLock::new(
                config
                    .permission_mode
                    .unwrap_or_else(|| "default".to_string()),
            )),
            model: Arc::new(RwLock::new(
                config.model.unwrap_or_else(|| "gpt-5.3-codex".to_string()),
            )),
            thinking_level: Arc::new(RwLock::new("medium".to_string())),
            cli_path: config.cli_path,
            app_handle,
            thread_id: Arc::new(RwLock::new(resume_session_id)),
            messages: Arc::new(RwLock::new(initial_messages)),
            active_process: Arc::new(Mutex::new(None)),
            in_flight: Arc::new(AtomicBool::new(false)),
            current_turn_id: Arc::new(RwLock::new(None)),
            aborted_turn_id: Arc::new(RwLock::new(None)),
        })
    }

    async fn execute_message(
        session_id: String,
        turn_id: String,
        folder_path: String,
        cli_path: Option<String>,
        message: String,
        model: Arc<RwLock<String>>,
        thinking_level: Arc<RwLock<String>>,
        permission_mode: Arc<RwLock<String>>,
        thread_id: Arc<RwLock<Option<String>>>,
        messages: Arc<RwLock<Vec<Message>>>,
        app_handle: AppHandle,
        active_process: Arc<Mutex<Option<Child>>>,
        in_flight: Arc<AtomicBool>,
        current_turn_id: Arc<RwLock<Option<String>>>,
        aborted_turn_id: Arc<RwLock<Option<String>>>,
    ) {
        let mut command = Command::new(cli_path.unwrap_or_else(|| "codex".to_string()));
        let mode = permission_mode.read().await.clone();
        let current_model = model.read().await.clone();
        let current_thinking = thinking_level.read().await.clone();

        command
            .arg("exec")
            // Map UI permission presets to explicit Codex sandbox/approval settings.
            // We run with stdin=null, so we must avoid interactive approval flows.
            .args(codex_permission_args_for_mode(&mode))
            .arg("--json")
            .arg("--skip-git-repo-check")
            .arg("-m")
            .arg(current_model)
            .current_dir(&folder_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        // Pass reasoning effort if set
        if !current_thinking.is_empty() {
            command
                .arg("-c")
                .arg(format!("model_reasoning_effort=\"{}\"", current_thinking));
        }

        let resume_id = thread_id.read().await.clone();
        let resume_attempted = resume_id.is_some();
        if let Some(existing_thread_id) = resume_id {
            command.arg("resume").arg(existing_thread_id);
        }

        command.arg("--").arg(message.clone());

        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(err) => {
                let error_event = ChatEvent::Error {
                    message: format!("Failed to spawn codex CLI: {err}"),
                };
                emit_chat_event(
                    &app_handle,
                    Some(session_id.as_str()),
                    Some(turn_id.as_str()),
                    &error_event,
                );
                Self::release_turn_state(
                    turn_id.as_str(),
                    &active_process,
                    &in_flight,
                    &current_turn_id,
                )
                .await;
                return;
            }
        };

        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                let _ = child.start_kill();
                let error_event = ChatEvent::Error {
                    message: "Failed to capture codex stdout".to_string(),
                };
                emit_chat_event(
                    &app_handle,
                    Some(session_id.as_str()),
                    Some(turn_id.as_str()),
                    &error_event,
                );
                Self::release_turn_state(
                    turn_id.as_str(),
                    &active_process,
                    &in_flight,
                    &current_turn_id,
                )
                .await;
                return;
            }
        };

        let stderr = match child.stderr.take() {
            Some(stderr) => stderr,
            None => {
                let _ = child.start_kill();
                let error_event = ChatEvent::Error {
                    message: "Failed to capture codex stderr".to_string(),
                };
                emit_chat_event(
                    &app_handle,
                    Some(session_id.as_str()),
                    Some(turn_id.as_str()),
                    &error_event,
                );
                Self::release_turn_state(
                    turn_id.as_str(),
                    &active_process,
                    &in_flight,
                    &current_turn_id,
                )
                .await;
                return;
            }
        };

        *active_process.lock().await = Some(child);
        let session_init = ChatEvent::SessionInit {
            auth_type: "codex".to_string(),
        };
        emit_chat_event(
            &app_handle,
            Some(session_id.as_str()),
            Some(turn_id.as_str()),
            &session_init,
        );

        let app_for_stdout = app_handle.clone();
        let session_for_stdout = session_id.clone();
        let turn_for_stdout = turn_id.clone();
        let thread_for_stdout = thread_id.clone();
        let messages_for_stdout = messages.clone();
        let stdout_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            let mut finished = false;
            let mut saw_thread_started = false;
            let mut active_tools: HashMap<String, String> = HashMap::new();
            let mut assistant_parts: Vec<String> = Vec::new();
            let mut rollout_path = None;

            loop {
                let Some(line) = lines.next_line().await.unwrap_or(None) else {
                    break;
                };

                if line.trim().is_empty() {
                    continue;
                }

                let parsed = match serde_json::from_str::<Value>(&line) {
                    Ok(v) => v,
                    Err(_) => {
                        let text_event = ChatEvent::Text { content: line };
                        emit_chat_event(
                            &app_for_stdout,
                            Some(session_for_stdout.as_str()),
                            Some(turn_for_stdout.as_str()),
                            &text_event,
                        );
                        continue;
                    }
                };

                let kind = event_type(&parsed).unwrap_or_default();
                let current_item_kind =
                    first_string(&parsed, &[&["item", "type"], &["item_type"], &["kind"]])
                        .unwrap_or("");

                if kind == "thread.started" {
                    if let Some(id) =
                        first_string(&parsed, &[&["thread_id"], &["thread", "id"], &["id"]])
                    {
                        saw_thread_started = true;
                        *thread_for_stdout.write().await = Some(id.to_string());
                        rollout_path = find_rollout_path_for_thread(id);
                    }
                }

                if kind == "item.started" {
                    let item_kind = if current_item_kind.is_empty() {
                        "tool"
                    } else {
                        current_item_kind
                    };
                    let item_id = first_string(&parsed, &[&["item", "id"], &["item_id"], &["id"]])
                        .unwrap_or("item")
                        .to_string();

                    if item_kind.contains("reason") {
                        let thinking_content = clean_thinking_text(
                            first_string(&parsed, &[&["item", "text"]]).unwrap_or("Thinking"),
                        );
                        let thinking_start = ChatEvent::ThinkingStart {
                            thinking_id: item_id,
                            content: thinking_content.to_string(),
                        };
                        emit_chat_event(
                            &app_for_stdout,
                            Some(session_for_stdout.as_str()),
                            Some(turn_for_stdout.as_str()),
                            &thinking_start,
                        );
                    } else if item_kind == "agent_message" {
                        // Text messages are handled on item.completed, skip here
                    } else {
                        let raw_tool_type = first_string(
                            &parsed,
                            &[&["item", "type"], &["item", "name"], &["name"]],
                        )
                        .unwrap_or("command_execution");
                        // Normalize Codex tool types to match frontend config keys
                        let tool_type = match raw_tool_type {
                            "web_search_call" => "web_search".to_string(),
                            other => other.to_string(),
                        };
                        let target = first_string(
                            &parsed,
                            &[
                                &["item", "command"],
                                &["item", "query"],
                                &["item", "action", "query"],
                                &["item", "name"],
                            ],
                        )
                        .unwrap_or("")
                        .to_string();
                        active_tools.insert(item_id.clone(), tool_type.clone());
                        let tool_start = ChatEvent::ToolStart {
                            tool_use_id: item_id.clone(),
                            tool_type,
                            target,
                            status: "pending".to_string(),
                            input: None,
                        };
                        emit_chat_event(
                            &app_for_stdout,
                            Some(session_for_stdout.as_str()),
                            Some(turn_for_stdout.as_str()),
                            &tool_start,
                        );
                        let tool_running = ChatEvent::ToolStatusUpdate {
                            tool_use_id: item_id.clone(),
                            status: "running".to_string(),
                            permission_request_id: None,
                        };
                        emit_chat_event(
                            &app_for_stdout,
                            Some(session_for_stdout.as_str()),
                            Some(turn_for_stdout.as_str()),
                            &tool_running,
                        );
                    }
                }

                if kind == "item.completed" {
                    let item_id = first_string(&parsed, &[&["item", "id"], &["item_id"], &["id"]])
                        .unwrap_or("item")
                        .to_string();
                    let item_kind = if current_item_kind.is_empty() {
                        "tool"
                    } else {
                        current_item_kind
                    };

                    if item_kind.contains("reason") {
                        let thinking_content = clean_thinking_text(
                            first_string(&parsed, &[&["item", "text"]]).unwrap_or("Thinking"),
                        );
                        let thinking_start = ChatEvent::ThinkingStart {
                            thinking_id: item_id.clone(),
                            content: thinking_content.to_string(),
                        };
                        emit_chat_event(
                            &app_for_stdout,
                            Some(session_for_stdout.as_str()),
                            Some(turn_for_stdout.as_str()),
                            &thinking_start,
                        );
                        let thinking_end = ChatEvent::ThinkingEnd {
                            thinking_id: item_id,
                        };
                        emit_chat_event(
                            &app_for_stdout,
                            Some(session_for_stdout.as_str()),
                            Some(turn_for_stdout.as_str()),
                            &thinking_end,
                        );
                    } else if item_kind == "web_search_call" && !active_tools.contains_key(&item_id)
                    {
                        // web_search_call may arrive only as item.completed, emit start+end
                        let tool_type = "web_search".to_string();
                        let target = first_string(
                            &parsed,
                            &[&["item", "action", "query"], &["item", "query"]],
                        )
                        .unwrap_or("")
                        .to_string();
                        let tool_start = ChatEvent::ToolStart {
                            tool_use_id: item_id.clone(),
                            tool_type,
                            target,
                            status: "pending".to_string(),
                            input: None,
                        };
                        emit_chat_event(
                            &app_for_stdout,
                            Some(session_for_stdout.as_str()),
                            Some(turn_for_stdout.as_str()),
                            &tool_start,
                        );
                        let tool_end = ChatEvent::ToolEnd {
                            id: item_id.clone(),
                            status: "completed".to_string(),
                        };
                        emit_chat_event(
                            &app_for_stdout,
                            Some(session_for_stdout.as_str()),
                            Some(turn_for_stdout.as_str()),
                            &tool_end,
                        );
                    } else if item_kind == "file_change" && !active_tools.contains_key(&item_id) {
                        // file_change only arrives as item.completed, emit start+end
                        let tool_type = "file_change".to_string();
                        let target = parsed
                            .get("item")
                            .and_then(|v| v.get("changes"))
                            .and_then(Value::as_array)
                            .and_then(|arr| arr.first())
                            .and_then(|c| c.get("path"))
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        let tool_start = ChatEvent::ToolStart {
                            tool_use_id: item_id.clone(),
                            tool_type,
                            target,
                            status: "pending".to_string(),
                            input: None,
                        };
                        emit_chat_event(
                            &app_for_stdout,
                            Some(session_for_stdout.as_str()),
                            Some(turn_for_stdout.as_str()),
                            &tool_start,
                        );
                        let tool_end = ChatEvent::ToolEnd {
                            id: item_id.clone(),
                            status: "completed".to_string(),
                        };
                        emit_chat_event(
                            &app_for_stdout,
                            Some(session_for_stdout.as_str()),
                            Some(turn_for_stdout.as_str()),
                            &tool_end,
                        );
                    } else if active_tools.contains_key(&item_id) {
                        let tool_type = active_tools
                            .remove(&item_id)
                            .unwrap_or_else(|| item_kind.to_string());
                        let completed_status =
                            first_string(&parsed, &[&["item", "status"]]).unwrap_or("completed");
                        let exit_code = parsed
                            .get("item")
                            .and_then(|v| v.get("exit_code"))
                            .and_then(Value::as_i64);
                        let final_status =
                            final_tool_status(&tool_type, completed_status, exit_code);
                        let tool_end = ChatEvent::ToolEnd {
                            id: item_id.clone(),
                            status: final_status.to_string(),
                        };
                        emit_chat_event(
                            &app_for_stdout,
                            Some(session_for_stdout.as_str()),
                            Some(turn_for_stdout.as_str()),
                            &tool_end,
                        );
                    }
                }

                let should_emit_text = !(current_item_kind.contains("reason")
                    || current_item_kind == "command_execution"
                    || current_item_kind == "web_search"
                    || current_item_kind == "web_search_call"
                    || current_item_kind == "file_change");

                if should_emit_text {
                    if let Some(text) = first_string(
                        &parsed,
                        &[
                            &["delta"],
                            &["text"],
                            &["content"],
                            &["item", "text"],
                            &["item", "content", "text"],
                            &["message", "content", "text"],
                        ],
                    ) {
                        if !text.is_empty() {
                            assistant_parts.push(text.to_string());
                            let text_event = ChatEvent::Text {
                                content: text.to_string(),
                            };
                            emit_chat_event(
                                &app_for_stdout,
                                Some(session_for_stdout.as_str()),
                                Some(turn_for_stdout.as_str()),
                                &text_event,
                            );
                        }
                    }
                }

                if kind == "turn.completed" {
                    let mut emitted_usage = false;

                    let rollout_for_turn = if let Some(path) = rollout_path.clone() {
                        Some(path)
                    } else {
                        let thread = thread_for_stdout.read().await.clone();
                        thread.as_deref().and_then(find_rollout_path_for_thread)
                    };

                    if let Some(rollout_file) = rollout_for_turn {
                        if let Some((latest, previous)) = latest_token_count_snapshot(&rollout_file)
                        {
                            let delta = previous
                                .map(|prev| latest.total_usage.saturating_sub(prev.total_usage))
                                .unwrap_or(latest.total_usage);

                            let token_usage = ChatEvent::TokenUsage {
                                total_tokens: delta.total_tokens,
                                context_tokens: Some(latest.last_input_tokens),
                                context_window: Some(latest.model_context_window),
                            };
                            emit_chat_event(
                                &app_for_stdout,
                                Some(session_for_stdout.as_str()),
                                Some(turn_for_stdout.as_str()),
                                &token_usage,
                            );
                            emitted_usage = true;
                        }
                    }

                    if !emitted_usage {
                        if let Some(total) = token_usage(&parsed) {
                            let token_usage = ChatEvent::TokenUsage {
                                total_tokens: total,
                                context_tokens: None,
                                context_window: None,
                            };
                            emit_chat_event(
                                &app_for_stdout,
                                Some(session_for_stdout.as_str()),
                                Some(turn_for_stdout.as_str()),
                                &token_usage,
                            );
                        }
                    }
                    finished = true;
                }

                if kind == "error" {
                    if let Some(err) =
                        first_string(&parsed, &[&["message"], &["error"], &["error", "message"]])
                    {
                        let error_event = ChatEvent::Error {
                            message: err.to_string(),
                        };
                        emit_chat_event(
                            &app_for_stdout,
                            Some(session_for_stdout.as_str()),
                            Some(turn_for_stdout.as_str()),
                            &error_event,
                        );
                    }
                }
            }

            let assistant_message = if finished {
                assistant_parts.join("")
            } else {
                String::new()
            };
            if !assistant_message.trim().is_empty() {
                let mut messages = messages_for_stdout.write().await;
                messages.push(Message {
                    id: Uuid::new_v4().to_string(),
                    role: "assistant".to_string(),
                    content: assistant_message,
                    timestamp: chrono::Utc::now().timestamp(),
                });
            }

            (finished, saw_thread_started)
        });

        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Some(line) = lines.next_line().await.unwrap_or(None) {
                if !line.trim().is_empty() {
                    #[cfg(debug_assertions)]
                    eprintln!("[codex stderr] {}", line.trim());
                }
            }
        });

        let status = loop {
            let mut guard = active_process.lock().await;
            let Some(child) = guard.as_mut() else {
                break None;
            };
            match child.try_wait() {
                Ok(Some(status)) => {
                    *guard = None;
                    break Some(status);
                }
                Ok(None) => {}
                Err(err) => {
                    let error_event = ChatEvent::Error {
                        message: format!("Failed to poll codex process: {err}"),
                    };
                    emit_chat_event(
                        &app_handle,
                        Some(session_id.as_str()),
                        Some(turn_id.as_str()),
                        &error_event,
                    );
                    *guard = None;
                    break None;
                }
            }
            drop(guard);
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        };

        let (_finished, saw_thread_started) = stdout_task.await.unwrap_or((false, false));
        let _ = stderr_task.await;

        if aborted_turn_id.read().await.as_deref() == Some(turn_id.as_str()) {
            *aborted_turn_id.write().await = None;
            Self::release_turn_state(
                turn_id.as_str(),
                &active_process,
                &in_flight,
                &current_turn_id,
            )
            .await;
            return;
        }

        if resume_attempted && !saw_thread_started {
            *thread_id.write().await = None;
        }

        match status {
            Some(exit) if exit.success() => {
                // Emit completion only after the process has fully exited.
                // This prevents queued sends from racing with lingering process teardown.
                let complete_event = ChatEvent::Complete;
                emit_chat_event(
                    &app_handle,
                    Some(session_id.as_str()),
                    Some(turn_id.as_str()),
                    &complete_event,
                );
            }
            Some(exit) => {
                let error_event = ChatEvent::Error {
                    message: format!("Codex exited with status {}", exit),
                };
                emit_chat_event(
                    &app_handle,
                    Some(session_id.as_str()),
                    Some(turn_id.as_str()),
                    &error_event,
                );
            }
            None => {
                let error_event = ChatEvent::Error {
                    message: "Codex process ended unexpectedly".to_string(),
                };
                emit_chat_event(
                    &app_handle,
                    Some(session_id.as_str()),
                    Some(turn_id.as_str()),
                    &error_event,
                );
            }
        }

        Self::release_turn_state(
            turn_id.as_str(),
            &active_process,
            &in_flight,
            &current_turn_id,
        )
        .await;
    }
}

#[async_trait]
impl BackendSession for CodexSession {
    fn session_id(&self) -> &str {
        &self.id
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::Codex
    }

    fn folder_path(&self) -> &str {
        &self.folder_path
    }

    async fn send_message(&self, message: &str, turn_id: Option<&str>) -> Result<(), BackendError> {
        if self.in_flight.swap(true, Ordering::SeqCst) {
            return Err(BackendError {
                message: "Codex session is busy".to_string(),
                recoverable: true,
            });
        }

        let turn_id = turn_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        *self.current_turn_id.write().await = Some(turn_id.clone());

        let timestamp = chrono::Utc::now().timestamp();
        {
            let mut messages = self.messages.write().await;
            messages.push(Message {
                id: Uuid::new_v4().to_string(),
                role: "user".to_string(),
                content: message.to_string(),
                timestamp,
            });
        }

        let session_id = self.id.clone();
        let folder_path = self.folder_path.clone();
        let cli_path = self.cli_path.clone();
        let model = self.model.clone();
        let thinking_level = self.thinking_level.clone();
        let permission_mode = self.permission_mode.clone();
        let thread_id = self.thread_id.clone();
        let app_handle = self.app_handle.clone();
        let active_process = self.active_process.clone();
        let in_flight = self.in_flight.clone();
        let current_turn_id = self.current_turn_id.clone();
        let aborted_turn_id = self.aborted_turn_id.clone();
        let messages = self.messages.clone();
        let message = message.to_string();

        tokio::spawn(async move {
            Self::execute_message(
                session_id,
                turn_id,
                folder_path,
                cli_path,
                message,
                model,
                thinking_level,
                permission_mode,
                thread_id,
                messages,
                app_handle,
                active_process,
                in_flight,
                current_turn_id,
                aborted_turn_id,
            )
            .await;
        });

        Ok(())
    }

    async fn abort(&self) -> Result<(), BackendError> {
        let active_turn = self.current_turn_id.read().await.clone();
        if let Some(turn_id) = active_turn.as_ref() {
            *self.aborted_turn_id.write().await = Some(turn_id.clone());
        }

        let mut guard = self.active_process.lock().await;
        if let Some(child) = guard.as_mut() {
            let _ = child.start_kill();
        }
        *guard = None;
        self.in_flight.store(false, Ordering::SeqCst);
        *self.current_turn_id.write().await = None;

        let abort_complete = ChatEvent::AbortComplete {
            session_id: self.id.clone(),
        };
        emit_chat_event(
            &self.app_handle,
            Some(self.id.as_str()),
            active_turn.as_deref(),
            &abort_complete,
        );
        Ok(())
    }

    async fn cleanup(&self) {
        let _ = self.abort().await;
    }

    async fn get_permission_mode(&self) -> String {
        self.permission_mode.read().await.clone()
    }

    async fn set_permission_mode(&self, mode: String) -> Result<(), BackendError> {
        *self.permission_mode.write().await = mode;
        Ok(())
    }

    async fn get_model(&self) -> String {
        self.model.read().await.clone()
    }

    async fn set_model(&self, model: String) -> Result<(), BackendError> {
        *self.model.write().await = model;
        Ok(())
    }

    async fn set_thinking_level(&self, level: String) -> Result<(), BackendError> {
        *self.thinking_level.write().await = level;
        Ok(())
    }

    async fn get_messages(&self) -> Vec<Message> {
        self.messages.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::{clean_thinking_text, codex_permission_args_for_mode, final_tool_status};

    #[test]
    fn codex_permission_args_default_mode() {
        assert_eq!(
            codex_permission_args_for_mode("default"),
            vec![
                "--sandbox".to_string(),
                "read-only".to_string(),
                "-c".to_string(),
                "approval_policy=\"never\"".to_string(),
            ]
        );
    }

    #[test]
    fn codex_permission_args_accept_edits_mode() {
        assert_eq!(
            codex_permission_args_for_mode("acceptEdits"),
            vec![
                "--sandbox".to_string(),
                "workspace-write".to_string(),
                "-c".to_string(),
                "approval_policy=\"never\"".to_string(),
            ]
        );
    }

    #[test]
    fn codex_permission_args_bypass_mode() {
        assert_eq!(
            codex_permission_args_for_mode("bypassPermissions"),
            vec!["--dangerously-bypass-approvals-and-sandbox".to_string()]
        );
    }

    #[test]
    fn codex_permission_args_unknown_mode_falls_back_to_default() {
        assert_eq!(
            codex_permission_args_for_mode("someFutureMode"),
            vec![
                "--sandbox".to_string(),
                "read-only".to_string(),
                "-c".to_string(),
                "approval_policy=\"never\"".to_string(),
            ]
        );
    }

    #[test]
    fn clean_thinking_text_strips_wrapping_bold_markers() {
        assert_eq!(
            clean_thinking_text("**Planning command execution updates**"),
            "Planning command execution updates"
        );
    }

    #[test]
    fn clean_thinking_text_keeps_normal_text() {
        assert_eq!(clean_thinking_text("Thinking..."), "Thinking...");
    }

    #[test]
    fn final_tool_status_for_command_requires_zero_exit_code() {
        assert_eq!(
            final_tool_status("command_execution", "completed", Some(0)),
            "completed"
        );
        assert_eq!(
            final_tool_status("command_execution", "completed", Some(1)),
            "error"
        );
        assert_eq!(
            final_tool_status("command_execution", "completed", None),
            "error"
        );
    }

    #[test]
    fn final_tool_status_for_non_command_uses_item_status() {
        assert_eq!(
            final_tool_status("web_search", "completed", None),
            "completed"
        );
        assert_eq!(final_tool_status("web_search", "failed", None), "error");
    }
}
