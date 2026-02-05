# Backend Implementation Guide

Practical guide for implementing CLI-based backends in Caipi, based on protocol analysis and experimentation.

## Executive Summary

| Aspect | Claude Code | Codex CLI | Impact |
|--------|-------------|-----------|--------|
| Permission flow | Bidirectional JSON | Preset flags only | Codex cannot have per-operation prompts |
| Thinking events | In message content | Separate item type | Different parsing logic |
| Tool events | Unified tool_use | Distinct item types | Separate handlers needed |
| Session resume | `--resume <id>` | `resume <id>` | Syntax difference only |

**Key finding:** Codex exec mode does not support runtime permission requests. All permissions must be configured before execution via `--sandbox` and approval flags.

---

## Phase 1: Claude CLI Wrapper

Build a direct CLI wrapper for Claude Code to prove the pattern before adding Codex.

### 1.1 Module Structure

```
src-tauri/src/backends/
├── mod.rs                 # Public exports
├── types.rs               # Backend/BackendSession traits (exists)
├── session.rs             # BackendSession trait (exists)
├── process.rs             # NEW: CliProcess utilities
├── claude/
│   ├── mod.rs
│   ├── adapter.rs         # ClaudeBackend (exists)
│   ├── sdk.rs             # Current SDK-based (rename from agent.rs)
│   └── cli/               # NEW
│       ├── mod.rs         # ClaudeCliSession
│       ├── process.rs     # Process management
│       ├── protocol.rs    # Message types
│       └── control.rs     # Control protocol
└── codex/                 # FUTURE
    ├── mod.rs
    ├── adapter.rs
    └── cli/
        ├── mod.rs
        └── protocol.rs
```

### 1.2 Shared CliProcess

```rust
// src-tauri/src/backends/process.rs

use tokio::process::{Child, Command};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};

pub struct CliProcess {
    child: Child,
    stdin: BufWriter<tokio::process::ChildStdin>,
    stdout_reader: tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
}

impl CliProcess {
    pub async fn spawn(
        program: &str,
        args: &[&str],
        cwd: &Path,
        env: &[(&str, &str)],
    ) -> Result<Self, Error>;

    pub async fn write_line(&mut self, json: &str) -> Result<(), Error>;

    pub async fn read_line(&mut self) -> Option<Result<String, Error>>;

    pub fn kill(&mut self) -> Result<(), Error>;

    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>, Error>;
}
```

### 1.3 Claude Protocol Types

```rust
// src-tauri/src/backends/claude/cli/protocol.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ClaudeMessage {
    #[serde(rename = "system")]
    System(SystemMessage),
    #[serde(rename = "assistant")]
    Assistant(AssistantMessage),
    #[serde(rename = "user")]
    User(UserMessage),
    #[serde(rename = "result")]
    Result(ResultMessage),
    #[serde(rename = "control_request")]
    ControlRequest(IncomingControlRequest),
    #[serde(rename = "control_response")]
    ControlResponse(ControlResponseMessage),
}

#[derive(Debug, Deserialize)]
pub struct SystemMessage {
    pub subtype: String,  // "init"
    pub session_id: String,
    pub cwd: String,
    pub model: String,
    #[serde(rename = "permissionMode")]
    pub permission_mode: String,
    #[serde(rename = "apiKeySource")]
    pub api_key_source: String,
    pub tools: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct AssistantMessage {
    pub message: AssistantMessageContent,
    pub session_id: String,
}

#[derive(Debug, Deserialize)]
pub struct AssistantMessageContent {
    pub model: String,
    pub id: String,
    pub content: Vec<ContentBlock>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
    },
}

// NOTE: These UserMessage types are simplified placeholders.
// Real JSONL has deeper nesting: message.content[].tool_use_id, etc.
// Capture actual JSONL fixtures to refine these structs.
#[derive(Debug, Deserialize)]
pub struct UserMessage {
    pub message: UserMessageContent,
    pub tool_use_result: Option<ToolUseResult>,
}

#[derive(Debug, Deserialize)]
pub struct ToolUseResult {
    #[serde(rename = "type")]
    pub result_type: String,  // "create", "update", "delete"
    #[serde(rename = "filePath")]
    pub file_path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResultMessage {
    pub subtype: String,  // "success" or "error"
    pub is_error: bool,
    pub session_id: String,
    pub result: String,
    pub duration_ms: u64,
    pub num_turns: u32,
    pub total_cost_usd: Option<f64>,
}

// Control protocol types
#[derive(Debug, Deserialize)]
pub struct IncomingControlRequest {
    pub request_id: String,
    pub request: ControlRequestData,
}

#[derive(Debug, Deserialize)]
pub struct ControlRequestData {
    pub subtype: String,  // "hook_callback"
    pub callback_id: Option<String>,
    pub input: Option<HookInput>,
}

#[derive(Debug, Deserialize)]
pub struct HookInput {
    pub hook_event_name: String,
    pub tool_name: Option<String>,
    pub tool_input: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct ControlResponse {
    #[serde(rename = "type")]
    pub msg_type: String,  // "control_response"
    pub response: ControlResponseData,
}

#[derive(Debug, Serialize)]
pub struct ControlResponseData {
    pub subtype: String,  // "success"
    pub request_id: String,
    pub response: HookResponse,
}

#[derive(Debug, Serialize)]
pub struct HookResponse {
    #[serde(rename = "hookSpecificOutput")]
    pub hook_specific_output: HookSpecificOutput,
}

#[derive(Debug, Serialize)]
pub struct HookSpecificOutput {
    #[serde(rename = "hookEventName")]
    pub hook_event_name: String,
    #[serde(rename = "permissionDecision")]
    pub permission_decision: String,  // "allow", "deny"
}
```

### 1.4 Control Protocol Implementation

```rust
// src-tauri/src/backends/claude/cli/control.rs

impl ClaudeCliSession {
    /// Initialize the CLI with hooks for permission handling
    async fn initialize(&mut self) -> Result<(), Error> {
        let request_id = Uuid::new_v4().to_string();
        let init_request = json!({
            "type": "control_request",
            "request_id": request_id,  // Required for correlating response
            "request": {
                "subtype": "initialize",
                "hooks": {
                    "PreToolUse": [{
                        "matcher": "*",
                        "hookCallbackIds": ["pretool_0"]
                    }],
                    "PostToolUse": [{
                        "matcher": "*",
                        "hookCallbackIds": ["posttool_0"]
                    }]
                }
            }
        });

        self.process.write_line(&init_request.to_string()).await?;

        // Wait for init response
        while let Some(line) = self.process.read_line().await {
            let msg: ClaudeMessage = serde_json::from_str(&line?)?;
            if matches!(msg, ClaudeMessage::ControlResponse(_)) {
                break;
            }
        }

        Ok(())
    }

    /// Handle incoming control request (permission callback)
    async fn handle_control_request(
        &mut self,
        request: IncomingControlRequest,
        permission_callback: impl Fn(&str, &serde_json::Value) -> BoxFuture<'static, bool>,
    ) -> Result<(), Error> {
        let input = request.request.input.ok_or(Error::MissingInput)?;
        let tool_name = input.tool_name.unwrap_or_default();
        let tool_input = input.tool_input.unwrap_or(json!({}));

        // Call permission callback (this triggers UI prompt)
        let allowed = permission_callback(&tool_name, &tool_input).await;

        // Send response
        let response = ControlResponse {
            msg_type: "control_response".to_string(),
            response: ControlResponseData {
                subtype: "success".to_string(),
                request_id: request.request_id,
                response: HookResponse {
                    hook_specific_output: HookSpecificOutput {
                        hook_event_name: input.hook_event_name,
                        permission_decision: if allowed { "allow" } else { "deny" }.to_string(),
                    },
                },
            },
        };

        self.process.write_line(&serde_json::to_string(&response)?).await?;
        Ok(())
    }
}
```

### 1.5 Event Conversion

```rust
// src-tauri/src/backends/claude/cli/mod.rs

impl ClaudeCliSession {
    fn convert_to_chat_event(&self, msg: ClaudeMessage) -> Vec<ChatEvent> {
        match msg {
            ClaudeMessage::System(sys) if sys.subtype == "init" => {
                vec![ChatEvent::SessionInit {
                    auth_type: match sys.api_key_source.as_str() {
                        "none" => "Claude AI Subscription".to_string(),
                        _ => "Anthropic API Key".to_string(),
                    },
                }]
            }

            ClaudeMessage::Assistant(ast) => {
                let mut events = vec![];

                for block in ast.message.content {
                    match block {
                        ContentBlock::Text { text } => {
                            events.push(ChatEvent::Text { content: text });
                        }
                        ContentBlock::ToolUse { id, name, input } => {
                            // NOTE: Current Caipi SDK integration emits ToolStart from
                            // PreToolUse hook (not here) to show awaiting_permission/running
                            // transitions. For CLI direct, you may want to either:
                            // 1. Emit from hook callback (like SDK does), or
                            // 2. Emit here but dedupe with hook-triggered events
                            events.push(ChatEvent::ToolStart {
                                tool_use_id: id,
                                tool_type: name.clone(),
                                target: extract_tool_target(&name, &input),
                                status: "pending".to_string(),
                                input: Some(input),
                            });
                        }
                        ContentBlock::Thinking { thinking } => {
                            events.push(ChatEvent::ThinkingStart {
                                thinking_id: uuid::Uuid::new_v4().to_string(),
                                content: thinking,
                            });
                        }
                    }
                }

                if let Some(usage) = ast.message.usage {
                    events.push(ChatEvent::TokenUsage {
                        total_tokens: usage.input_tokens + usage.output_tokens,
                    });
                }

                events
            }

            ClaudeMessage::User(usr) => {
                let mut events = vec![];

                if let Some(result) = usr.tool_use_result {
                    // Extract tool_use_id from message content
                    if let Some(content) = usr.message.content.first() {
                        if let Some(id) = content.get("tool_use_id").and_then(|v| v.as_str()) {
                            events.push(ChatEvent::ToolEnd {
                                id: id.to_string(),
                                status: "completed".to_string(),
                            });
                        }
                    }
                }

                events
            }

            ClaudeMessage::Result(res) => {
                if res.is_error {
                    vec![ChatEvent::Error {
                        message: res.result,
                    }]
                } else {
                    vec![ChatEvent::Complete]
                }
            }

            _ => vec![],
        }
    }
}
```

---

## Phase 2: Validation

### 2.1 Feature Flag

```rust
// src-tauri/src/backends/claude/mod.rs

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClaudeImplementation {
    Sdk,       // Current: uses claude-agent-sdk-rs
    CliDirect, // New: direct CLI wrapper
}

impl ClaudeBackend {
    pub fn new(implementation: ClaudeImplementation) -> Self {
        Self { implementation }
    }
}
```

### 2.2 Validation Test Cases

Run both implementations with identical inputs:

| Test | Expected |
|------|----------|
| Simple text response | Identical text output |
| Tool use (Write file) | Same file created, same events |
| Permission prompt | Both trigger UI, both respond correctly |
| Session resume | Both continue from same point |
| Abort mid-stream | Both stop cleanly |
| Error handling | Same error types |

---

## Phase 3: Codex Backend

### 3.1 Protocol Types

```rust
// src-tauri/src/backends/codex/cli/protocol.rs

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum CodexEvent {
    #[serde(rename = "thread.started")]
    ThreadStarted { thread_id: String },

    #[serde(rename = "turn.started")]
    TurnStarted,

    #[serde(rename = "item.started")]
    ItemStarted { item: CodexItem },

    #[serde(rename = "item.completed")]
    ItemCompleted { item: CodexItem },

    #[serde(rename = "turn.completed")]
    TurnCompleted { usage: CodexUsage },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum CodexItem {
    #[serde(rename = "reasoning")]
    Reasoning {
        id: String,
        text: String,
    },

    #[serde(rename = "command_execution")]
    CommandExecution {
        id: String,
        command: String,
        aggregated_output: String,
        exit_code: Option<i32>,
        status: String,  // "in_progress", "completed", "failed"
    },

    #[serde(rename = "file_change")]
    FileChange {
        id: String,
        changes: Vec<FileChangeEntry>,
        status: String,
    },

    #[serde(rename = "agent_message")]
    AgentMessage {
        id: String,
        text: String,
    },
}

#[derive(Debug, Deserialize)]
pub struct FileChangeEntry {
    pub path: String,
    pub kind: String,  // "create", "update", "delete"
}

#[derive(Debug, Deserialize)]
pub struct CodexUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: Option<u64>,
    pub output_tokens: u64,
}
```

### 3.2 Event Conversion

```rust
// src-tauri/src/backends/codex/cli/mod.rs

impl CodexCliSession {
    fn convert_to_chat_event(&self, event: CodexEvent) -> Vec<ChatEvent> {
        match event {
            CodexEvent::ThreadStarted { thread_id } => {
                self.thread_id = Some(thread_id);
                vec![ChatEvent::SessionInit {
                    auth_type: "OpenAI API".to_string(),
                }]
            }

            CodexEvent::ItemStarted { item } => {
                match item {
                    CodexItem::CommandExecution { id, command, .. } => {
                        vec![ChatEvent::ToolStart {
                            tool_use_id: id,
                            tool_type: "Bash".to_string(),
                            target: command,
                            status: "running".to_string(),
                            input: None,
                        }]
                    }
                    _ => vec![],
                }
            }

            CodexEvent::ItemCompleted { item } => {
                match item {
                    CodexItem::Reasoning { id, text } => {
                        vec![
                            ChatEvent::ThinkingStart {
                                thinking_id: id.clone(),
                                content: text,
                            },
                            ChatEvent::ThinkingEnd { thinking_id: id },
                        ]
                    }

                    CodexItem::CommandExecution { id, status, .. } => {
                        vec![ChatEvent::ToolEnd {
                            id,
                            status: if status == "completed" { "completed" } else { "error" }.to_string(),
                        }]
                    }

                    CodexItem::FileChange { id, changes, .. } => {
                        let target = changes
                            .first()
                            .map(|c| c.path.clone())
                            .unwrap_or_default();

                        vec![
                            ChatEvent::ToolStart {
                                tool_use_id: id.clone(),
                                tool_type: "Edit".to_string(),
                                target,
                                status: "completed".to_string(),
                                input: None,
                            },
                            ChatEvent::ToolEnd {
                                id,
                                status: "completed".to_string(),
                            },
                        ]
                    }

                    CodexItem::AgentMessage { text, .. } => {
                        vec![ChatEvent::Text { content: text }]
                    }
                }
            }

            CodexEvent::TurnCompleted { usage } => {
                vec![
                    ChatEvent::TokenUsage {
                        total_tokens: usage.input_tokens + usage.output_tokens,
                    },
                    ChatEvent::Complete,
                ]
            }

            _ => vec![],
        }
    }
}
```

### 3.3 Permission Handling (Limited)

```rust
// src-tauri/src/backends/codex/cli/mod.rs

impl CodexCliSession {
    /// Build CLI arguments based on permission mode
    fn build_args(&self, config: &SessionConfig) -> Vec<String> {
        let mut args = vec![
            "exec".to_string(),
            "--json".to_string(),
        ];

        // Map Caipi permission modes to Codex sandbox/approval settings
        match config.permission_mode.as_str() {
            "bypassPermissions" => {
                // Full auto mode - no prompts
                args.push("--dangerously-bypass-approvals-and-sandbox".to_string());
            }
            "acceptEdits" => {
                // Auto-approve file edits, sandbox commands
                args.push("--full-auto".to_string());
            }
            "default" | _ => {
                // Workspace-write sandbox - allows file edits in project
                // Note: Cannot do per-operation prompts programmatically in exec mode
                // (approval policies exist but manifest as TTY prompts, causing hangs)
                args.push("--sandbox".to_string());
                args.push("workspace-write".to_string());
            }
        }

        if let Some(model) = &config.model {
            args.push("--model".to_string());
            args.push(model.clone());
        }

        if let Some(dir) = &config.folder_path {
            args.push("-C".to_string());
            args.push(dir.clone());
        }

        args
    }
}
```

**Important limitation:** Codex has no programmatic permission callback mechanism in exec mode. While approval policies (`-a/--ask-for-approval`) exist, they manifest as TTY prompts that would cause hangs in a GUI wrapper. The permission UI will not appear for Codex sessions - permissions must be configured upfront via sandbox mode selection.

---

## Key Differences Summary

| Feature | Claude CLI Wrapper | Codex CLI Wrapper |
|---------|-------------------|-------------------|
| Permission prompts | Yes (control protocol) | No (programmatic callbacks unavailable) |
| Thinking visibility | Parse from content blocks | Separate `reasoning` events |
| ToolStart source | PreToolUse hook callback (preferred) or `tool_use` blocks | `item.started` events |
| ToolEnd source | `user` message with `tool_result` | `item.completed` events |
| Session ID | `session_id` in messages | `thread_id` in thread.started |
| Resume command | `--resume <id>` | `resume <id>` subcommand |

---

## Testing Checklist

- [ ] Claude CLI: Simple text response
- [ ] Claude CLI: Tool use with permission prompt
- [ ] Claude CLI: Permission denied flow
- [ ] Claude CLI: Session resume
- [ ] Claude CLI: Abort handling
- [ ] Claude CLI: Extended thinking
- [ ] Claude CLI: Error handling
- [ ] Codex CLI: Simple text response
- [ ] Codex CLI: Command execution
- [ ] Codex CLI: File change events
- [ ] Codex CLI: Reasoning events
- [ ] Codex CLI: Session resume
- [ ] Codex CLI: Different sandbox modes

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Control protocol changes | Pin CLI version, monitor changelogs |
| Codex protocol undocumented | Use experimental data, test extensively |
| Permission UX mismatch | Document Codex limitations in UI |
| Performance differences | Benchmark, optimize parsing |

---

## References

- [CLI Protocol Reference](./codex_vs_claude.md) - Detailed event formats
- [Claude Agent SDK Source](https://github.com/pietz/claude-agent-sdk-rs) - SDK implementation reference
