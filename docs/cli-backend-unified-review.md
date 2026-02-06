# Claude CLI Backend Unified Review

Date: 2026-02-05
Scope: staged Claude CLI backend rollout (`claudecli`) plus follow-up fixes.

## Method
- Compared SDK backend (`src-tauri/src/claude/agent.rs`) vs CLI backend (`src-tauri/src/backends/claude/cli_adapter.rs`).
- Validated claims against current source, not just staged diff.
- Ran protocol sanity checks with real CLI output.
- Ran `cargo test -q` in `src-tauri` (pass).

## Executive Summary
- The architecture is solid and extensible.
- Two previously critical issues were fixed (abort context loss, duplicate ToolEnd).
- Token handling for `claudecli` now uses assistant usage fields to represent context usage.

## Findings (Sorted by Priority)

### P1

### P2

2. No process-crash signaling / stale session process state risk
- Status: Open
- Sources: Both reviews
- Evidence:
  - Reader task exits silently on EOF/parse path with no crash event: `src-tauri/src/backends/claude/cli_adapter.rs:295`
  - `self.process` is only cleared in explicit abort/cleanup paths: `src-tauri/src/backends/claude/cli_adapter.rs:919`, `src-tauri/src/backends/claude/cli_adapter.rs:940`
- Impact:
  - Silent backend death; next send can fail opaquely.
- Recommendation:
  - Monitor child exit (`wait`) and emit `ChatEvent::Error`; atomically clear process/stdin on unexpected exit.

3. Thinking level toggle is a no-op in CLI backend
- Status: Open
- Sources: Both reviews
- Evidence:
  - CLI backend no-op: `src-tauri/src/backends/claude/cli_adapter.rs:969`
  - UI exposes thinking toggle globally: `src/lib/components/chat/MessageInput.svelte:57`
- Impact:
  - User-visible parity gap; setting appears to work but does nothing.
- Recommendation:
  - Either implement a real control, or disable/hide toggle for `claudecli` and communicate limitation.

4. Interactive tools denied at runtime instead of disallowed at model planning time
- Status: Open
- Sources: External review + validated
- Evidence:
  - SDK disallows upfront: `src-tauri/src/claude/agent.rs:227`
  - CLI denies in permission hook path: `src-tauri/src/claude/hooks.rs:59`
- Impact:
  - Extra tool attempts and token waste; potential model confusion.
- Recommendation:
  - Pass CLI-level disallow flags if available; otherwise keep denial but tune prompting/system instructions.

5. Permission mode updates mid-session may not fully match spawn-time bypass semantics
- Status: Open (edge-case risk)
- Sources: External review + validated
- Evidence:
  - `set_permission_mode` only updates local state: `src-tauri/src/backends/claude/cli_adapter.rs:955`
  - `--dangerously-skip-permissions` only set on spawn based on mode: `src-tauri/src/backends/claude/cli_adapter.rs:209`
- Impact:
  - Potential divergence for CLI-internal permission behavior after mode switch.
- Recommendation:
  - Option A: respawn process on mode switch for strong parity.
  - Option B: document mode-change semantics as hook-level only.

### P3

6. Generic error messaging on result errors
- Status: Open
- Sources: External review + validated
- Evidence:
  - Emits hardcoded message: `src-tauri/src/backends/claude/cli_adapter.rs:832`
  - `ResultEvent` omits potential `result/errors` payload fields: `src-tauri/src/claude/cli_protocol.rs:252`
- Impact:
  - Poor troubleshooting UX.
- Recommendation:
  - Extend `ResultEvent` parsing for actual error text and surface it.

7. Session ID fallback sends literal `"default"`
- Status: Open (low confidence)
- Sources: External review + validated
- Evidence:
  - `session_id: ...unwrap_or("default")`: `src-tauri/src/backends/claude/cli_adapter.rs:409`
- Impact:
  - Protocol ambiguity if CLI treats this specially.
- Recommendation:
  - Omit `session_id` when unknown rather than sending synthetic value.

8. Duplicate `PermissionDecision` type names across modules
- Status: Open (maintainability)
- Sources: External review + validated
- Evidence:
  - Semantic permission decision enum in hooks: `src-tauri/src/claude/hooks.rs:32`
  - Protocol enum in CLI protocol legacy section: `src-tauri/src/claude/cli_protocol.rs:698`
- Impact:
  - Developer confusion/import mistakes.
- Recommendation:
  - Rename one type or remove legacy protocol types from active module.

9. `setting_sources` parity gap (SDK explicit, CLI implicit)
- Status: Open (low severity)
- Sources: External review + validated
- Evidence:
  - SDK sets `SettingSource::User, Project`: `src-tauri/src/claude/agent.rs:234`
  - CLI path has no equivalent explicit flag.
- Impact:
  - Mostly documentation/parity risk if CLI defaults change.
- Recommendation:
  - Document intended assumptions or pass explicit equivalent if supported.

10. `--verbose` parsing risk
- Status: Not reproduced, keep as watch-item
- Sources: External review
- Evidence:
  - CLI adapter requires `--verbose`: `src-tauri/src/backends/claude/cli_adapter.rs:199`
  - Real probe emitted JSON-only stdout in tested runs.
- Impact:
  - Low; parse errors are already tolerated.
- Recommendation:
  - Keep parse-error telemetry; revisit only if production logs show frequent parse drops.

## Resolved Findings

1. Abort no longer drops context on next message
- Status: Resolved
- Evidence:
  - Respawn after abort now resumes if prior CLI session id exists: `src-tauri/src/backends/claude/cli_adapter.rs:890`

2. Duplicate `ToolEnd` from `PostToolUse` + `ToolResult`
- Status: Resolved
- Evidence:
  - `PostToolUse` now ACK-only: `src-tauri/src/backends/claude/cli_adapter.rs:635`

3. Opus mapping/version parity
- Status: Resolved
- Evidence:
  - SDK maps `opus` to `claude-opus-4-6`: `src-tauri/src/claude/agent.rs:95`
  - CLI probe confirmed model availability (`model":"claude-opus-4-6"`).

4. Missing `ToolEnd` when tools complete via `user.tool_result`
- Status: Resolved
- Evidence:
  - `CliEvent::User` now handled: `src-tauri/src/backends/claude/cli_adapter.rs`
  - `ToolEnd` deduped across user/assistant tool_result variants via `active_tools`.

5. Token usage semantics for `claudecli` context indicator
- Status: Resolved
- Decision:
  - `claudecli` now emits token usage from assistant `usage` per API call as:
    `input_tokens + cache_read_input_tokens + cache_creation_input_tokens`.
  - It no longer overwrites this with cumulative `result` totals.
- Evidence:
  - CLI adapter now computes usage from `AssistantEvent.message.usage`.
  - Result-event token emission removed in `src-tauri/src/backends/claude/cli_adapter.rs`.

## Rejected / Not Applicable

1. Missing `claude:permission_request` event
- Status: Rejected
- Reason:
  - Current architecture uses `ChatEvent::ToolStatusUpdate { status: "awaiting_permission" }` in both SDK and CLI flows, and frontend is wired for that path.
  - Evidence SDK hooks: `src-tauri/src/claude/hooks.rs:229`
  - Evidence CLI hooks: `src-tauri/src/backends/claude/cli_adapter.rs:759`

2. `--replay-user-messages` absence is inherently broken
- Status: Rejected as a standalone claim
- Reason:
  - In tested CLI version, `user` tool_result events appear without that flag in normal `-p` stream-json output.
  - Real issue is that `CliEvent::User` is currently ignored in adapter logic.

## Suggested Fix Order
1. Add child-exit/error propagation and clear stale process handles (P2).
2. Decide and implement backend-specific thinking toggle behavior (P2).
