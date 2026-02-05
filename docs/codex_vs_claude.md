# CLI Protocol Reference

Comprehensive protocol documentation for Claude Code and Codex CLIs, based on experimental verification (February 2026).

## Quick Reference

| Aspect | Claude Code | Codex CLI |
|--------|-------------|-----------|
| Non-interactive mode | `claude -p` | `codex exec` |
| JSON streaming | `--output-format stream-json --verbose` | `--json` |
| Input format | `--input-format stream-json` (stdin) | Prompt as argument or stdin with `-` |
| Permission control | `--permission-mode` + bidirectional JSON callbacks | `--sandbox` + preset (no programmatic callbacks) |
| Session ID source | `session_id` in init event | `thread_id` in thread.started event |

---

## 1. Process Spawning

### Claude Code

```bash
claude -p \
  --output-format stream-json \
  --verbose \
  --input-format stream-json \
  [--model <model>] \
  [--permission-mode <mode>] \
  [--resume <session-id>]
```

**Required flags for GUI wrapper:**
- `-p` (print mode) - non-interactive
- `--output-format stream-json` - structured event output
- `--verbose` - required when using stream-json

**Optional flags:**
- `--input-format stream-json` - enables bidirectional JSON communication
- `--model sonnet|opus|haiku` - model selection
- `--permission-mode default|acceptEdits|bypassPermissions` - permission handling
- `--resume <session-id>` - continue previous session

### Codex CLI

```bash
codex exec \
  --json \
  [--model <model>] \
  [--sandbox <mode>] \
  [--full-auto] \
  [-C <directory>] \
  "prompt"
```

**Required flags for GUI wrapper:**
- `--json` - JSONL event output

**Optional flags:**
- `--model gpt-5-codex|gpt-5|gpt-5.1-codex-max` - model selection
- `--sandbox read-only|workspace-write|danger-full-access` - sandbox level
- `--full-auto` - convenience preset (workspace-write + on-request approvals)
- `-C <dir>` - working directory

---

## 2. Event Stream Format

### Claude Code Events

All events are JSON objects, one per line on stdout.

#### System Init Event
```json
{
  "type": "system",
  "subtype": "init",
  "cwd": "/path/to/project",
  "session_id": "uuid",
  "tools": ["Read", "Write", "Bash", ...],
  "model": "claude-opus-4-5-20251101",
  "permissionMode": "default",
  "apiKeySource": "none|api_key",
  "claude_code_version": "2.1.31"
}
```

#### Assistant Message Event
```json
{
  "type": "assistant",
  "message": {
    "model": "claude-opus-4-5-20251101",
    "id": "msg_xxx",
    "role": "assistant",
    "content": [
      {"type": "text", "text": "Response text..."},
      {"type": "tool_use", "id": "toolu_xxx", "name": "Write", "input": {...}}
    ],
    "usage": {
      "input_tokens": 100,
      "output_tokens": 50,
      "cache_read_input_tokens": 1000,
      "cache_creation_input_tokens": 500
    }
  },
  "session_id": "uuid"
}
```

#### Tool Result Event (User Message)
```json
{
  "type": "user",
  "message": {
    "role": "user",
    "content": [
      {
        "tool_use_id": "toolu_xxx",
        "type": "tool_result",
        "content": "Tool output text"
      }
    ]
  },
  "tool_use_result": {
    "type": "create|update|delete",
    "filePath": "/path/to/file",
    "content": "file content if applicable"
  }
}
```

#### Result Event
```json
{
  "type": "result",
  "subtype": "success|error",
  "is_error": false,
  "duration_ms": 5000,
  "num_turns": 3,
  "result": "Final response text",
  "session_id": "uuid",
  "total_cost_usd": 0.05,
  "usage": {...}
}
```

### Codex CLI Events

All events are JSON objects, one per line on stdout.

#### Thread Started Event
```json
{
  "type": "thread.started",
  "thread_id": "uuid"
}
```

#### Turn Started Event
```json
{
  "type": "turn.started"
}
```

#### Reasoning Event (Thinking)
```json
{
  "type": "item.completed",
  "item": {
    "id": "item_0",
    "type": "reasoning",
    "text": "Thinking about the approach..."
  }
}
```

#### Command Execution Events
```json
// Started
{
  "type": "item.started",
  "item": {
    "id": "item_1",
    "type": "command_execution",
    "command": "/bin/zsh -lc 'ls -la'",
    "aggregated_output": "",
    "exit_code": null,
    "status": "in_progress"
  }
}

// Completed (success)
{
  "type": "item.completed",
  "item": {
    "id": "item_1",
    "type": "command_execution",
    "command": "/bin/zsh -lc 'ls -la'",
    "aggregated_output": "file1.txt\nfile2.txt\n",
    "exit_code": 0,
    "status": "completed"
  }
}

// Completed (failure)
{
  "type": "item.completed",
  "item": {
    "id": "item_1",
    "type": "command_execution",
    "command": "/bin/zsh -lc 'cat nonexistent.txt'",
    "aggregated_output": "cat: nonexistent.txt: No such file or directory\n",
    "exit_code": 1,
    "status": "failed"
  }
}
```

#### File Change Event
```json
{
  "type": "item.completed",
  "item": {
    "id": "item_3",
    "type": "file_change",
    "changes": [
      {"path": "/path/to/file.txt", "kind": "update"}
    ],
    "status": "completed"
  }
}
```

#### Agent Message Event
```json
{
  "type": "item.completed",
  "item": {
    "id": "item_5",
    "type": "agent_message",
    "text": "I've completed the task..."
  }
}
```

#### Turn Completed Event
```json
{
  "type": "turn.completed",
  "usage": {
    "input_tokens": 14172,
    "cached_input_tokens": 4352,
    "output_tokens": 127
  }
}
```

---

## 3. Event Mapping to ChatEvent

| ChatEvent | Claude Code Source | Codex CLI Source |
|-----------|-------------------|------------------|
| `SessionInit` | `system` + `subtype: init` | `thread.started` |
| `Text` | `assistant` message with text content blocks | `item.completed` with `type: agent_message` |
| `ThinkingStart/End` | `assistant` message with thinking blocks | `item.completed` with `type: reasoning` |
| `ToolStart` | PreToolUse hook callback* or `tool_use` blocks | `item.started` with `type: command_execution/file_change` |
| `ToolEnd` | `user` message with `tool_result` | `item.completed` with status |
| `TokenUsage` | `assistant.message.usage` or `result.usage` | `turn.completed.usage` |
| `Complete` | `result` with `subtype: success` | `turn.completed` |
| `Error` | `result` with `subtype: error` | `item.completed` with `status: failed` (partial) |

*Current Caipi emits `ToolStart` from the PreToolUse hook (not from parsing `tool_use` blocks) to show `awaiting_permission`/`running` status transitions before execution.

---

## 4. Permission System

### Claude Code: Bidirectional JSON Protocol

Claude Code supports **runtime permission requests** via bidirectional JSON communication.

#### Initialization (Send to stdin)
```json
{
  "type": "control_request",
  "request_id": "init_001",
  "request": {
    "subtype": "initialize",
    "hooks": {
      "PreToolUse": [
        {
          "matcher": "*",
          "hookCallbackIds": ["hook_0"]
        }
      ]
    }
  }
}
```

**Note:** `request_id` is required to correlate the response. The CLI will send back a `control_response` with the same `request_id`.

#### Permission Request (Received from stdout)
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
      "tool_input": {"command": "rm -rf /tmp/test"}
    }
  }
}
```

#### Permission Response (Send to stdin)
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

**Permission decisions:** `"allow"`, `"deny"`, `"ask"` (escalate to user)

### Codex CLI: No Programmatic Permission Callbacks

Codex CLI does **NOT** support programmatic runtime permission callbacks in exec mode. While approval policies exist (`-a/--ask-for-approval`), they manifest as TTY prompts that would cause hangs in a GUI wrapper.

**Sandbox modes:**
- `read-only` (default) - no file modifications
- `workspace-write` - can modify files in workspace
- `danger-full-access` - full system access

**Approval policies (available but not usable programmatically):**
- `untrusted` - ask for everything (TTY prompt, causes hang)
- `on-failure` - ask only on failures (TTY prompt)
- `on-request` - model decides when to ask (TTY prompt)
- `never` - never ask (safe for automation)

**For GUI wrappers:** Use `--full-auto` (workspace-write + on-request) or configure sandbox level upfront. Per-operation permission prompts cannot be serviced programmatically - the CLI will hang waiting for TTY input.

---

## 5. Session Management

### Claude Code

```bash
# New session
claude -p "prompt"

# Resume by ID
claude -p --resume "session-id" "follow-up"

# Continue last session
claude -p --continue "follow-up"
```

**Session ID extraction:**
```javascript
// From init event
const sessionId = event.session_id; // when event.type === "system" && event.subtype === "init"

// Or from result event
const sessionId = event.session_id; // when event.type === "result"
```

### Codex CLI

```bash
# New session
codex exec "prompt"

# Resume by ID
codex exec resume <thread-id> "follow-up"

# Continue last session
codex exec resume --last "follow-up"
```

**Thread ID extraction:**
```javascript
// From thread.started event
const threadId = event.thread_id; // when event.type === "thread.started"
```

---

## 6. Abort/Interrupt

### Claude Code

Send SIGINT to the process or use the SDK's abort mechanism. The CLI responds with a result event with `stop_reason` indicating interruption.

### Codex CLI

Send SIGINT to the process. The turn will complete with current state.

---

## 7. Error Handling

### Claude Code

Errors appear in the result event:
```json
{
  "type": "result",
  "subtype": "error",
  "is_error": true,
  "result": "Error message"
}
```

### Codex CLI

Tool failures appear in item.completed events:
```json
{
  "type": "item.completed",
  "item": {
    "type": "command_execution",
    "status": "failed",
    "exit_code": 1,
    "aggregated_output": "Error output"
  }
}
```

Session-level errors may appear as separate error events (not fully documented).

---

## 8. Model Selection

### Claude Code

| Model | Flag Value | Notes |
|-------|------------|-------|
| Claude Opus 4.5 | `opus` or `claude-opus-4-5-20251101` | Most capable |
| Claude Sonnet 4.5 | `sonnet` or `claude-sonnet-4-5-20250929` | Default |
| Claude Haiku 4.5 | `haiku` or `claude-haiku-4-5-20250929` | Fastest |

### Codex CLI

| Model | Flag Value | Notes |
|-------|------------|-------|
| GPT-5 Codex | `gpt-5-codex` | Default (macOS/Linux) |
| GPT-5 | `gpt-5` | Default (Windows) |
| GPT-5.1 Codex Max | `gpt-5.1-codex-max` | Most capable |
| GPT-4.1 Mini | `gpt-4.1-mini` | Lightweight |

---

## 9. Known Differences

| Feature | Claude Code | Codex CLI |
|---------|-------------|-----------|
| Programmatic permissions | Yes (control_request/response callbacks) | No (approval policies exist but require TTY) |
| Thinking/reasoning events | In assistant message content | Separate `reasoning` item type |
| Tool types | Unified tool_use blocks | Distinct item types (command_execution, file_change, etc.) |
| ToolStart source | PreToolUse hook or tool_use parsing | item.started events |
| Session persistence | Optional (`--no-session-persistence`) | Always enabled |
| Working directory | Inherited from shell | Can set with `-C` flag |
| MCP integration | Built-in with `--mcp-config` | Via `codex mcp` commands |

---

## 10. Verified CLI Versions

- Claude Code: 2.1.31
- Codex CLI: 0.96.0

Last verified: February 2026
