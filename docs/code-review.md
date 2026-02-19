# Caipi Code Review — Unified Report

**Date:** 2026-02-18
**Scope:** Full codebase (frontend + backend), excluding reference dirs
**Agents:** 5 Opus reviewers (Rust backend, frontend-backend contracts, frontend state/reactivity, cross-cutting, pattern consistency)

## Executive Summary

**70 raw findings** across 5 agents, **~46 unique** after deduplication. The codebase is well-structured overall — the backend abstraction, event system, and store architecture are sound. The main themes are:

1. **Resource lifecycle gaps** — spawned tasks without abort handles, fire-and-forget cleanup
2. **Svelte 5 reactivity traps** — native Map/Set in `$state`, $effect patterns
3. **Security surface** — CSP disabled, wildcard fs scope
4. **Code duplication** — Codex tool parsing exists in two places
5. **Inconsistent patterns** — Claude vs Codex adapter symmetry, error handling strategies

Three findings were independently discovered by 3 agents (Codex duplication, dead commands, storage TOCTOU), giving high confidence in those.

---

## Findings by Priority

### P0 — Critical (1)

| # | Finding | Risk | Effort | Obj. | Domain |
|---|---------|------|--------|------|--------|
| 1 | ~~**Codex tool-parsing duplicated between `cli_protocol.rs` and `sessions.rs`** — 3 verbatim helper functions + 1 structural duplicate. Protocol changes must be made in 2 places; forgetting one causes silent history/live divergence.~~ ✅ | M | L | H | Rust |

*Found by 3 agents independently.*

---

### P1 — High (10)

| # | Finding | Risk | Effort | Obj. | Domain |
|---|---------|------|--------|------|--------|
| 2 | ~~**Spawned tokio tasks have no abort handles** — stdout/stderr readers and process monitors are `tokio::spawn`ed without storing `JoinHandle`s. After abort, stale events can still be emitted.~~ ✅ | M | M | H | Rust |
| 3 | ~~**Window close cleanup is fire-and-forget** — cleanup task spawned but app exits immediately. Orphaned CLI processes continue running and consuming API credits.~~ ✅ | M | M | H | Rust |
| 4 | ~~**CSP disabled** (`"csp": null`) — no Content Security Policy. Combined with `@html` markdown rendering, XSS would have full Tauri IPC access.~~ ✅ | M | M | H | Security |
| 5 | ~~**Wildcard filesystem scope** (`"path": "**"`) — frontend has read/write access to every file on the system. Should be scoped to `$HOME`/`$APPDATA`.~~ ✅ | M | M | H | Security |
| 6 | ~~**`$state` with native `Map`** — `tools` in chat store uses `new Map()` instead of `SvelteMap`. Mutations via `.set()` won't trigger reactivity. Currently safe by accident (full replacement pattern), but fragile.~~ ✅ | M | M | H | Frontend |
| 7 | ~~**`$state` with native `Set`** — `expanded` in files store and `SessionPicker` uses `new Set()` instead of `SvelteSet`. Same trap as above.~~ ✅ | M | L | H | Frontend |
| 8 | ~~**`SessionPicker.$effect` runs `loadSessions()` unconditionally** — no explicit dependencies, should be `onMount`. May cause spurious re-fetches on unrelated state changes.~~ ✅ | L | L | H | Frontend |
| 9 | ~~**`cleanup()` semantics differ between backends** — Claude silently kills process; Codex emits `AbortComplete` event and sleeps 500ms. Callers expect quiet teardown but Codex triggers UI state changes.~~ ✅ | M | M | H | Rust |
| 10 | ~~**Two different `PermissionDecision` enums in same module tree** — `hooks.rs` and `cli_protocol.rs` both define `PermissionDecision` with different shapes. Requires disambiguation at import sites.~~ ✅ | L | L | H | Rust |
| 11 | ~~**`send_control_response` returns `String` error** while all other I/O in the same adapter returns `BackendError`. Forces inconsistent error handling at call sites.~~ ✅ | L | L | H | Rust |

---

### P2 — Medium (22)

| # | Finding | Risk | Effort | Obj. | Domain |
|---|---------|------|--------|------|--------|
| 12 | ~~**11 Tauri commands registered but never invoked from frontend** — `check_cli_status`, `check_cli_installed`, `check_cli_authenticated`, `check_backend_cli_*`, `reset_onboarding`, `set_default_folder`, `get_default_backend`, `get_cli_path`, `set_cli_path`, `get_session_messages`. Expands IPC attack surface ~30% unnecessarily.~~ ✅ | L | L | H | Cross |
| 13 | ~~**Corrupt `data.json` bricks the app** — parse error returns `Err` instead of `AppData::default()`. No self-healing; user must manually delete the file.~~ ✅ | L | L | M | Rust |
| 14 | ~~**Codex `send_message` leaks oneshot channel** — `turn/start` response receiver immediately dropped but sender stays in `pending_requests` forever. Minor per-turn memory leak.~~ ✅ | L | L | H | Rust |
| 15 | **Permission mode and tool status are bare `String`s** — compared against magic literals like `"bypassPermissions"`. Typos are silent bugs. Should be enums. | L | M | M | Rust |
| 16 | ~~**Storage reads don't hold lock** — several read operations skip the mutex while writes hold it. `get_license()` migration has TOCTOU window.~~ ✅ | L | L | H | Rust |
| 17 | **Claude abort blocks tokio task 500ms** — sequential sleep in abort path delays UI response. Codex has same pattern. | L | L | H | Rust |
| 18 | ~~**Codex process monitor holds mutex across sleep** — lock not explicitly dropped before `tokio::time::sleep`. Claude adapter does drop it.~~ ✅ | L | L | H | Rust |
| 19 | **Codex `agent_message` completion may double-emit text** — comment says "if we haven't received deltas" but no actual check exists. | M | L | M | Rust |
| 20 | **`respond_permission` ignores `sessionId`** — parameter accepted but unused (`_session_id`). No validation that request belongs to the claimed session. | M | M | H | Contract |
| 21 | ~~**`StartupInfo.backendCliPaths` typed optional but always present** — Rust field always serializes (as `{}`), TS marks it `?`.~~ ✅ | L | L | H | Contract |
| 22 | **`ToolCallStack` $effect reads and writes `revealedIds`** — needs `untrack()` to prevent unnecessary re-runs. Two instances. | M | M | M | Frontend |
| 23 | ~~**Missing `.catch()` on permission/model/thinking API calls** in `MessageInput` — unhandled promise rejections on backend failure.~~ ✅ | L | L | H | Frontend |
| 24 | **Unstable `{#each}` keys** — `groupedStreamItems` keyed by array index. Insertions cause incorrect DOM reuse. | L | L | H | Frontend |
| 25 | **No test coverage for `files.svelte.ts` store** — `updateChildren` does recursive tree traversal, prime for edge case bugs. | L | M | H | Frontend |
| 26 | ~~**Missing OS plugin permission** in capabilities — `platform()` from `@tauri-apps/plugin-os` used but no `os:` permission declared.~~ ✅ | M | L | H | Config |
| 27 | ~~**Opener scope missing Windows paths** — `/tmp/**` invalid on Windows; projects outside `$HOME` blocked.~~ ✅ | M | L | H | Platform |
| 28 | **Extensive `#[allow(dead_code)]` on Backend trait** — 11 annotations on designed-but-unused abstraction layer. Parallel code paths exist in `setup.rs`. | L | L | H | Rust |
| 29 | **SessionInit emitted once (Claude) vs every turn (Codex)** — different lifecycle semantics for same event type. | M | M | H | Pattern |
| 30 | ~~**Codex stderr only logged in debug builds** — Claude always logs. Production Codex issues silently swallowed.~~ ✅ | L | L | H | Pattern |
| 31 | **Updater store uses closure pattern** while all other stores use class pattern — sole outlier. | L | M | L | Pattern |
| 32 | **Model registry defined in both frontend and backend** — `BackendCapabilities.available_models` never used by frontend, which has its own config. | L | L | M | Pattern |
| 33 | **Codex approval response silently swallows I/O errors** — manual serialization with `let _ = ...` instead of using `write_line` method. | L | L | H | Pattern |

---

### P3 — Low (13)

| # | Finding | Risk | Effort | Obj. | Domain |
|---|---------|------|--------|------|--------|
| 34 | **`SystemSubtype`/`ResultSubtype` enums defined but never used** — string comparisons used instead. | L | L | H | Rust |
| 35 | **`ChatEvent` serde has redundant rename on `AbortComplete`** — renames to what it already is. Missing explicit `rename_all`. | L | L | M | Rust |
| 36 | **`claude` module unnecessarily `pub`** in `lib.rs` — only used internally. | L | L | L | Rust |
| 37 | **`Option<T>` serializes as `null` but TS uses `T \| undefined`** — works via truthiness checks but incorrect under `strictNullChecks`. | L | L | M | Contract |
| 38 | **`AbortComplete` carries redundant `sessionId`** in both envelope and payload — payload copy never read. | L | L | M | Contract |
| 39 | **`list_directory` bypasses centralized API wrapper** — only command invoked directly from components. | L | L | M | Contract |
| 40 | **Double state reset on abort** — `finalize()` then `setStreaming(false)` does the same cleanup twice. | L | L | H | Frontend |
| 41 | **`ThemeStore.destroy()` is dead code** — exists but never called on singleton. | L | L | H | Frontend |
| 42 | **`className` property in `ToolConfig` unused** — defined on 20+ configs, never read by any component. | L | L | H | Frontend |
| 43 | **Multiple `setTimeout` calls without cleanup** — `LicenseEntry`, `SetupWizard`, `ToolCallStack` set timers without clearing on destroy. | L | L | M | Frontend |
| 44 | **`Date.now()` used for stream item IDs** — theoretical collision if two items created in same millisecond. | L | L | M | Frontend |
| 45 | **Hardcoded path in release script** — `/Users/pietz/Private/caipi.ai` works only on one machine. | L | L | H | Build |
| 46 | **Inconsistent error display patterns** — each screen handles errors differently (inline text, icons, styled containers). | L | M | L | Frontend |

---

## Recommended Action Plan

### Phase 1: Quick Wins (< 1 day, high impact)

These are low-effort, high-objectivity fixes:

| # | Action | Effort | Status |
|---|--------|--------|--------|
| 1 | Deduplicate Codex tool-parsing into shared module | L | ✅ Done |
| 6-7 | Replace native `Map`/`Set` with `SvelteMap`/`SvelteSet` in stores | L-M | ✅ Done |
| 8 | Change `SessionPicker.$effect` to `onMount` | L | ✅ Done |
| 12 | Remove 11 unused Tauri commands from `generate_handler!` | L | ✅ Done |
| 14 | Don't register `pending_request` for `turn/start` (no response needed) | L | ✅ Done |
| 16 | Add lock to storage reads, especially `get_license` migration | L | ✅ Done |
| 18 | Add explicit `drop(guard)` in Codex process monitor | L | ✅ Done |
| 23 | Add `.catch()` to fire-and-forget API calls in `MessageInput` | L | ✅ Done |
| 10 | Rename `cli_protocol::PermissionDecision` to `CliPermissionDecision` | L | ✅ Done |
| 11 | Change `send_control_response` to return `BackendError` | L | ✅ Done |

### Phase 2: Important Improvements (1-2 days)

| # | Action | Effort |
|---|--------|--------|
| 4-5 | Add CSP and scope filesystem permissions ✅ | M |
| 2 | Store `JoinHandle`s and abort spawned tasks on cleanup ✅ | M |
| 3 | Implement proper window close cleanup (wait for sessions) ✅ | M |
| 9 | Separate `cleanup()` from `abort()` in Codex adapter ✅ | M |
| 15 | Define `PermissionMode` and `ToolStatus` enums | M |
| 13 | Return `AppData::default()` on corrupt `data.json` ✅ | L |
| 26 | Add `os:default` permission to capabilities ✅ | L |
| 27 | Fix opener scope for Windows paths ✅ | L |
| 30 | Make Codex stderr logging match Claude (always on) ✅ | L |

### Phase 3: Polish (when convenient)

Everything P3, plus lower-priority P2 items like test coverage, updater store pattern, model registry cleanup.

---

## Cross-Agent Convergence

Three findings were independently discovered by multiple agents, indicating high confidence:

| Finding | Agents | Agreement |
|---------|--------|-----------|
| Codex tool-parsing duplication | Rust, Cross-Cutting, Patterns | 3/5 |
| Dead Tauri commands | Contracts, Cross-Cutting | 2/5 |
| Storage TOCTOU on reads | Rust, Cross-Cutting | 2/5 |
| `pub mod claude` | Rust, Cross-Cutting | 2/5 |

---

## What's Working Well

The agents also noted areas of strength:

- **ChatEvent contract is solid** — all variants match between Rust enums and TypeScript unions, including field names, types, and optionality
- **Tauri invoke parameter names all match** — automatic camelCase-to-snake_case conversion verified for all 20+ commands
- **Backend model names are consistent** — frontend config and Rust capabilities define matching model IDs
- **Permission roundtrip is correct** — UUID-based request IDs, 60s timeout, abort handling all implemented
- **Event envelope filtering works** — session/turn ID gating prevents stale events
- **Storage atomic writes** — temp file + persist pattern prevents partial writes
- **Test infrastructure** — replay-based testing, behavioral tests, and good fixture coverage for the event system
