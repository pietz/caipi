# Caipi Architecture Review Findings

> Last Updated: 2026-01-28
> Status: Phase 1 complete

---

## Summary

Findings from architecture reviews of the Caipi codebase. Prioritized for practical impact.

**Priority:** P0 (critical) → P1 (high) → P2 (medium) → P3 (low)
**Effort:** S (<1hr) → M (1-4hr) → L (4-8hr)

---

## Completed Fixes

### 1. Extended Thinking Toggle [FIXED]

**Files:** `src/lib/components/chat/MessageInput.svelte:138-150`

**Problem:** UI toggle updated local state but never called backend.

**Solution:** Connected toggle to call `api.setExtendedThinking()` after state update.

---

### 2. Session Path Encoding Collisions [FIXED]

**Files:** `src-tauri/src/commands/sessions.rs`

**Problem:** `/Users/foo-bar` and `/Users/foo/bar` both encoded to `-Users-foo-bar`.

**Solution:** Added `verify_project_path()` that checks `projectPath` in `sessions-index.json` before returning sessions. No data migration needed.

**Tests:** 6 new tests covering matching, collision, and edge cases.

---

### 3. CLI Auth Check Stubbed [FIXED]

**Files:** `src-tauri/src/commands/setup.rs`, `src/lib/components/onboarding/SetupWizard.svelte`

**Problem:** Auth check always returned `true`, hiding auth failures until first message.

**Solution:** Check for `ANTHROPIC_API_KEY` env var or `~/.claude/.credentials.json`. UI now shows auth status with instructions.

**Tests:** 3 new tests for credential file detection.

---

## High Priority (P1)

### 5. Frontend/Backend API Types Out of Sync

**Files:** `src/lib/api/types.ts`, `src/routes/+page.svelte`

**Problem:** TS types don't match Rust structs. Code uses `as unknown as` casts.
- TS: `{ needsOnboarding, lastFolder }`
- Rust: `{ onboarding_completed, cli_status, last_folder }`

**Fix:** Add `#[serde(rename_all = "camelCase")]` to Rust structs, update TS types to match.

| Effort | Priority |
|--------|----------|
| M | P1 |

---

### 6. Tool Target Extraction Duplicated

**Files:**
- `src-tauri/src/claude/hooks.rs:62`
- `src-tauri/src/claude/tool_utils.rs`
- `src-tauri/src/commands/sessions.rs:195`

**Problem:** Same extraction logic in 3 places. Will drift.

**Fix:** Consolidate into `tool_utils.rs`, update call sites.

| Effort | Priority |
|--------|----------|
| S | P1 |

---

### 7. Duplicate truncate_str Function

**Files:**
- `src-tauri/src/claude/tool_utils.rs:4`
- `src-tauri/src/commands/sessions.rs:185`

**Fix:** Move to single location.

| Effort | Priority |
|--------|----------|
| S | P1 |

---

### 8. CLI Path Discovery Fragile

**Files:** `src-tauri/src/commands/setup.rs:32`

**Problem:** Uses `/bin/zsh -l -c which claude`. Can fail in app bundles or with non-zsh shells.

**Fix:**
1. Allow user to configure CLI path in settings
2. Pass via `ClaudeAgentOptions.cli_path`

| Effort | Priority |
|--------|----------|
| M | P1 |

---

## Needs Verification

### 9. Claude Settings/Permissions Parsing

**Files:** `src-tauri/src/claude/settings.rs`

**Claimed Issue:** Settings parser may be outdated vs Claude Code's current `permissions` schema.

**My Take:** Need to verify this is actually broken. The app may just be passing mode to SDK, not reimplementing permission logic.

**Action:** Test whether current implementation causes issues before investing effort.

| Effort | Priority |
|--------|----------|
| ? | Verify first |

---

### 10. Setting Sources Missing User Scope

**Files:** `src-tauri/src/claude/agent.rs`

**Claimed Issue:** Only loading `[Project, Local]` settings, missing `User` and `Managed`.

**My Take:** SDK may handle this internally. Test whether adding scopes changes behavior.

**Action:** Verify before changing.

| Effort | Priority |
|--------|----------|
| S | Verify first |

---

## Medium Priority (P2)

### 11. Sessions Accumulate in Memory

**Files:** `src-tauri/src/lib.rs`, `src-tauri/src/commands/chat.rs`

**Problem:** SessionStore only drains on app close.

**Fix:** Cleanup on folder switch or idle timeout.

| Effort | Priority |
|--------|----------|
| M | P2 |

---

### 12. No Structured Logging

**Files:** Throughout `src-tauri/src/`

**Problem:** Uses `eprintln!()`. No log levels.

**Fix:** Add `tracing` crate.

| Effort | Priority |
|--------|----------|
| M | P2 |

---

### 13. Unused events.rs Module

**Files:** `src-tauri/src/commands/events.rs`

**Problem:** Dead code with `#[allow(dead_code)]`.

**Fix:** Delete it.

| Effort | Priority |
|--------|----------|
| S | P2 |

---

### 14. Event Buffering Uses Global State

**Files:** `src/lib/utils/events.ts`

**Problem:** Module-level buffer. Fragile if multiple concurrent sessions.

**Fix:** Buffer per sessionId.

| Effort | Priority |
|--------|----------|
| S | P2 |

---

## Low Priority / Deferred

### 15. Model Names Hardcoded

Multiple places define model names. Not urgent - models don't change often.

### 16. Hook Closures Clone Heavily

Performance optimization. Not a correctness issue.

### 17. Cross-Platform (Windows)

Many Unix assumptions. Only invest if Windows is a real target.

### 18. Multi-Provider Architecture

Nice-to-have `AgentBackend` trait for Codex/Copilot support. YAGNI until second provider is imminent.

---

## Execution Order

### Done
1. ~~Extended thinking toggle~~
2. ~~Session path collisions~~
3. ~~CLI auth check~~

### Next Up
4. API types sync with camelCase (#5)
5. Consolidate tool utils (#6, #7)
6. CLI path configuration (#8)

### Verify First
7. Settings parsing (#9)
8. Setting sources (#10)

### Later
9. Session cleanup (#11)
10. Structured logging (#12)
11. Delete dead code (#13)

---

## Key Files

| Area | Files |
|------|-------|
| Frontend API | `src/lib/api/tauri.ts`, `src/lib/api/types.ts` |
| Frontend State | `src/lib/stores/*.svelte.ts` |
| Backend Commands | `src-tauri/src/commands/*.rs` |
| Claude Integration | `src-tauri/src/claude/agent.rs`, `hooks.rs`, `settings.rs` |
