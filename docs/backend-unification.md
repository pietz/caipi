# Backend Unification: CLI Wrapper Strategy

## Goal

Support multiple AI coding assistants (Claude Code, OpenAI Codex) through a unified backend architecture.

Currently, Caipi uses an unofficial Rust SDK (`claude-agent-sdk-rs`) to communicate with Claude Code CLI. To add Codex CLI support, we need to build our own CLI wrapper—no SDK exists for Codex. Rather than maintaining two different integration patterns (SDK for Claude, custom wrapper for Codex), we'll build a consistent CLI wrapper architecture for both.

**The end state:**
- Internal CLI wrappers for both Claude Code and Codex CLI
- Shared `BackendSession` trait with consistent event model
- No external SDK dependencies
- Easy to add future backends (Gemini, local models, etc.)

## Why Build Claude Wrapper First

Building the Claude wrapper first (not Codex) lets us:

1. **Use the SDK as a test oracle** - Compare wrapper output against known-good SDK behavior
2. **Isolate variables** - Debug CLI wrapping patterns without also debugging Codex syntax
3. **Zero downtime** - Keep the SDK working while building the replacement
4. **Proven pattern** - When Codex comes, only syntax differs, not architecture

---

## Phase 1: Internal Claude CLI Wrapper

### 1.1 Create the Wrapper Module

```
src-tauri/src/backends/claude/
├── mod.rs           # Public exports
├── adapter.rs       # BackendSession implementation (existing, keep)
├── sdk.rs           # Current SDK-based AgentSession (rename from agent.rs)
└── cli/
    ├── mod.rs       # New CLI wrapper
    ├── process.rs   # Process spawning, stdin/stdout management
    ├── protocol.rs  # JSON message types, parsing
    └── control.rs   # Control protocol (hooks, permissions)
```

### 1.2 CLI Protocol Implementation

The Claude CLI protocol (from SDK reverse-engineering):

**Spawn command:**
```bash
claude -p \
  --output-format stream-json \
  --verbose \
  --input-format stream-json \
  [--model <model>] \
  [--permission-mode <mode>] \
  [--resume <session-id>] \
  [--max-thinking-tokens <n>]
```

**Stdin (to CLI):**
```jsonl
{"type": "control_request", "request": {"subtype": "initialize", "hooks": {...}}}
{"type": "user", "message": {"role": "user", "content": "..."}, "session_id": "..."}
{"type": "control_response", "response": {"request_id": "...", "response": {...}}}
```

**Stdout (from CLI):**
```jsonl
{"type": "system", "subtype": "init", "session_id": "...", "cwd": "...", ...}
{"type": "assistant", "message": {"content": [...], "model": "..."}, ...}
{"type": "control_request", "request_id": "...", "request": {"subtype": "hook_callback", ...}}
{"type": "result", "subtype": "success", "duration_ms": ..., "session_id": "..."}
```

### 1.3 Event Mapping

| CLI Message | ChatEvent |
|-------------|-----------|
| `system` + `subtype: init` | Extract session_id, auth info |
| `assistant` with text blocks | `ChatEvent::Text` |
| `assistant` with `tool_use` | `ChatEvent::ToolStart` (via hooks) |
| `control_request` + `hook_callback` | Execute hook, respond via stdin |
| `user` with `tool_result` | `ChatEvent::ToolEnd` |
| `assistant` with thinking blocks | `ChatEvent::ThinkingStart/End` |
| `result` | `ChatEvent::Complete` |

### 1.4 Control Protocol for Permissions

When CLI needs permission, it sends:
```json
{
  "type": "control_request",
  "request_id": "req_123",
  "request": {
    "subtype": "hook_callback",
    "callback_id": "hook_0",
    "input": {
      "hook_event_name": "PreToolUse",
      "tool_name": "Bash",
      "tool_input": {"command": "rm -rf /"}
    }
  }
}
```

Wrapper responds via stdin:
```json
{
  "type": "control_response",
  "response": {
    "subtype": "success",
    "request_id": "req_123",
    "response": {
      "hookSpecificOutput": {
        "hookEventName": "PreToolUse",
        "permissionDecision": "allow"
      }
    }
  }
}
```

---

## Phase 2: Parallel Validation

### 2.1 Feature Flag

Add a setting to toggle between SDK and CLI wrapper:

```rust
// src-tauri/src/backends/claude/mod.rs
pub enum ClaudeImplementation {
    Sdk,      // Current: uses claude-agent-sdk-rs
    CliDirect // New: internal CLI wrapper
}
```

Expose via settings so we can A/B test during development.

### 2.2 Validation Checklist

Run both implementations with identical inputs, compare:

| Aspect | Validation Method |
|--------|-------------------|
| Session creation | Both return valid session_id |
| Text streaming | Identical text content and chunking |
| Tool events | Same tool names, inputs, outputs |
| Permission flow | Both trigger permission UI correctly |
| Abort handling | Both interrupt cleanly |
| Session resume | Both resume from same session_id |
| Extended thinking | Both handle thinking blocks |
| Error cases | Same error types and messages |

### 2.3 Logging for Comparison

```rust
// Emit raw CLI events for debugging
if cfg!(debug_assertions) {
    log::debug!("CLI raw: {}", raw_json_line);
    log::debug!("Mapped to: {:?}", chat_event);
}
```

---

## Phase 3: SDK Removal

Once validation passes:

1. Remove `claude-agent-sdk-rs` from `Cargo.toml`
2. Delete `sdk.rs` (old AgentSession)
3. Rename `cli/mod.rs` → primary implementation
4. Remove feature flag, make CLI wrapper the only path

---

## Phase 4: Codex CLI Integration

### 4.1 Create Codex Module

```
src-tauri/src/backends/codex/
├── mod.rs
├── adapter.rs    # BackendSession implementation
└── cli/
    ├── mod.rs
    ├── process.rs   # (mostly shared patterns from Claude)
    └── protocol.rs  # Codex-specific JSONL types
```

### 4.2 Codex CLI Protocol

**Spawn command:**
```bash
codex exec \
  --json \
  [--model <model>] \
  [--sandbox <mode>] \
  [--ask-for-approval <policy>] \
  "prompt"
```

**Stdout (JSONL events):**
```jsonl
{"type": "thread.started", "thread_id": "..."}
{"type": "turn.started"}
{"type": "item.started", "item": {"id": "...", "type": "command_execution", ...}}
{"type": "item.completed", "item": {"id": "...", "type": "agent_message", "text": "..."}}
{"type": "turn.completed", "usage": {"input_tokens": ..., "output_tokens": ...}}
```

### 4.3 Event Mapping (Codex → ChatEvent)

| Codex Event | ChatEvent |
|-------------|-----------|
| `thread.started` | Extract thread_id as session_id |
| `item.started` + `command_execution` | `ChatEvent::ToolStart` |
| `item.completed` + `agent_message` | `ChatEvent::Text` |
| `item.completed` + `command_execution` | `ChatEvent::ToolEnd` |
| `turn.completed` | `ChatEvent::Complete` |

### 4.4 Session Resume

```bash
# Continue last session
codex exec resume --last "follow up message"

# Resume specific thread
codex exec resume <thread_id> "follow up message"
```

---

## Shared Infrastructure

### Common Types

```rust
// src-tauri/src/backends/types.rs

pub enum BackendKind {
    Claude,
    Codex,
}

pub struct SessionConfig {
    pub backend: BackendKind,
    pub model: String,
    pub folder_path: String,
    pub resume_session_id: Option<String>,
    pub permission_mode: PermissionMode,
}

// ChatEvent enum (already exists, shared by both backends)
```

### Process Management Utilities

```rust
// src-tauri/src/backends/process.rs

pub struct CliProcess {
    child: tokio::process::Child,
    stdin: ChildStdin,
    stdout_lines: Lines<BufReader<ChildStdout>>,
}

impl CliProcess {
    pub async fn spawn(cmd: &str, args: &[&str], cwd: &Path) -> Result<Self>;
    pub async fn write_line(&mut self, json: &str) -> Result<()>;
    pub async fn read_line(&mut self) -> Option<Result<String>>;
    pub async fn kill(&mut self) -> Result<()>;
}
```

---

## Timeline

| Phase | Description | Dependency |
|-------|-------------|------------|
| 1 | Build Claude CLI wrapper | None |
| 2 | Parallel validation vs SDK | Phase 1 |
| 3 | Remove SDK | Phase 2 validated |
| 4 | Build Codex wrapper | Phase 3 (or parallel with Phase 2) |

---

## Success Criteria

- [ ] Claude CLI wrapper passes all validation checks against SDK
- [ ] No user-visible behavior changes after SDK removal
- [ ] Codex integration works with same `BackendSession` trait
- [ ] Frontend code unchanged (only backend swap)
- [ ] `claude-agent-sdk-rs` removed from dependencies
