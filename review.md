# Caipi Code Review

Full codebase review conducted 2025-02-12. Covers frontend (Svelte 5), backend (Rust/Tauri), project structure, and patterns.

**Priority levels:** P0 = critical/bug, P1 = high, P2 = medium, P3 = low/nice-to-have
**Effort:** S = small (< 30 min), M = medium (1-2 hours), L = large (half day+)
**Risk:** S = safe/isolated, M = moderate/touches multiple files, L = high/could break things

---

## P0 — Bugs

### 1. `$derived` vs `$derived.by` in ChatContainer

| Effort | Risk |
|--------|------|
| S | S |

`ChatContainer.svelte:201` uses `$derived(() => ...)` which returns a **function**, not a memoized value. Every template access re-executes the grouping logic. Should be `$derived.by(...)`.

### 2. Inconsistent markdown rendering (streaming vs finalized)

| Effort | Risk |
|--------|------|
| M | S |

`ChatContainer.svelte:277` uses raw `marked.parse()` while `ChatMessage.svelte` uses `marked` + `hljs` custom renderer. Streaming code blocks lack syntax highlighting and visually "pop" when a message finalizes. Fix: extract a shared `renderMarkdown()` utility used by both.

---

## P1 — High Priority

### 3. Dead components (~360 lines)

| Effort | Risk |
|--------|------|
| S | S |

Three components are never imported anywhere:
- `FolderPicker.svelte` (211 lines) — superseded by SessionPicker
- `LicenseInfo.svelte` (82 lines) — duplicated in Settings
- `Welcome.svelte` (67 lines) — superseded by SetupWizard

Delete all three.

### 4. `sessions.rs` is 1,864 lines and does too much

| Effort | Risk |
|--------|------|
| L | M |

Handles Claude session parsing, Codex session parsing, Codex tool extraction, session caching, and Tauri commands all in one file. Split into:
- `sessions/mod.rs` — shared types (`SessionInfo`, `ProjectSessions`), cache logic, Tauri command entry points
- `sessions/claude.rs` — Claude session parsing
- `sessions/codex.rs` — Codex session parsing + tool extraction

### 5. ~200 lines of duplicated Codex tool parsing

| Effort | Risk |
|--------|------|
| M | M |

These functions are copy-pasted between `backends/codex/adapter.rs` and `commands/sessions.rs`:
- `web_run_target_from_args` (62 lines, verbatim duplicate)
- `first_array_entry` (verbatim duplicate)
- `parse_function_arguments` / `parse_item_arguments` (same logic, different names)
- `codex_tool_from_payload` / `normalized_tool_from_item` (parallel implementations diverging)

Extract to a shared `codex_tools` module imported by both.

### 6. Two parallel `claude/` module trees

| Effort | Risk |
|--------|------|
| M | M |

`src/claude/` (protocol types, hooks, settings) and `src/backends/claude/` (adapter) exist side-by-side from when there were multiple Claude backends. Now that only the CLI backend remains, consolidate everything under `backends/claude/`. Also ~127 lines of dead legacy protocol types in `cli_protocol.rs:557-684` that should be removed.

### 7. Unused npm dependencies

| Effort | Risk |
|--------|------|
| S | S |

- `bits-ui` — zero imports found anywhere
- `tailwind-variants` — zero imports found anywhere

Remove both.

---

## P2 — Medium Priority

### 8. `execute_message` is a 590-line method

| Effort | Risk |
|--------|------|
| L | M |

`codex/adapter.rs:372-959` spawns a process, reads stdout/stderr, parses events, manages tool lifecycle, handles token usage, and manages process exit — all in one function. `handle_event` takes 14 parameters, `handle_pretool_hook` takes 11. Both suppress `clippy::too_many_arguments`. Create context structs and decompose into smaller named functions.

### 9. Duplicated frontend utilities

| Effort | Risk |
|--------|------|
| M | S |

| What | Where | Fix |
|------|-------|-----|
| `formatTime` | FolderPicker + SessionPicker | Extract `formatRelativeTime(date: Date)` to `$lib/utils/format.ts` |
| `copyToClipboard` | Welcome + SetupWizard | Extract to `$lib/utils/clipboard.ts` |
| License disconnect | Settings + LicenseInfo | Shared function or reuse component |
| Theme toggle button | SetupWizard + LicenseEntry | Extract `ThemeToggle.svelte` |
| Backend display names | SessionPicker + SetupWizard + Settings | `getBackendDisplayName()` utility |

### 10. Storage migration repeated 3 times

| Effort | Risk |
|--------|------|
| S | S |

The `claudecli` → `claude` key migration is copy-pasted across `get_cli_path()`, `get_backend_cli_paths()`, and `get_backend_cli_path()` in `storage/mod.rs`. Extract a single `ensure_claude_key_migrated()` helper.

### 11. NavigationBar near-identical macOS/Windows blocks

| Effort | Risk |
|--------|------|
| S | S |

`NavigationBar.svelte` has two ~50-line template blocks (macOS and Windows) that are nearly identical. The only difference is `data-tauri-drag-region` and a spacer div. Use conditional attributes in a single template.

### 12. ChatMessage global CSS should move to app.css

| Effort | Risk |
|--------|------|
| S | S |

`ChatMessage.svelte` lines 71-193 are 123 lines of `:global(.message-content ...)` rules. These are intentionally global (also used by ChatContainer for streaming text). Move to `app.css` or a dedicated `message-content.css` to make the global nature explicit.

### 13. Duplicate and dead TypeScript types

| Effort | Risk |
|--------|------|
| S | S |

- `CliStatus` defined identically in `api/types.ts` and `stores/app.svelte.ts` — remove the one in `api/types.ts`
- `CliInstallStatus` and `CliAuthStatus` in `api/types.ts` are never imported — remove

### 14. `events.ts` module-level mutable state

| Effort | Risk |
|--------|------|
| M | M |

`lineBuffer`, `flushTimer`, `onContentChange` are module-level `let` variables forming a hidden singleton. Works today but fragile — relies on manual `resetEventState()` calls. Consider encapsulating in a class or factory function to make state ownership explicit.

### 15. Debug console.log statements

| Effort | Risk |
|--------|------|
| S | S |

`FileTreeItem.svelte:55-64` has 3 debug `console.log` calls that should be removed.

---

## P3 — Nice to Have

### 16. `handleToolEndEvent` is a growing god function

| Effort | Risk |
|--------|------|
| M | S |

`events.ts:181-257` handles 5 tool-type-specific behaviors (Skill, TodoWrite, update_plan, TaskCreate, TaskUpdate) via if-chains. Each new tool type adds another block. Consider a registry/dispatch pattern.

### 17. `createSession` API has 6 positional params

| Effort | Risk |
|--------|------|
| S | M |

`tauri.ts:35` — `createSession(folderPath, permissionMode?, model?, resumeSessionId?, cliPath?, backend?)`. Four optional positional params are error-prone. Refactor to an options object.

### 18. Inconsistent store patterns

| Effort | Risk |
|--------|------|
| S | S |

`updater.svelte.ts` uses a closure/factory pattern while all other stores use the class pattern. Convert to class for consistency.

### 19. `app.svelte.ts` → `chat` coupling

| Effort | Risk |
|--------|------|
| M | M |

`app.svelte.ts` imports `chat` directly for `resumeSession` (line 3, 377). One-directional coupling that could become circular. If session lifecycle grows, extract to a dedicated `session.ts` utility.

### 20. Template boilerplate in static/

| Effort | Risk |
|--------|------|
| S | S |

`svelte.svg`, `tauri.svg`, `vite.svg` are unused Vite template files. Delete them.

### 21. Missing test coverage

| Effort | Risk |
|--------|------|
| M | S |

No tests for:
- `files.svelte.ts` — the `updateChildren` recursive tree update deserves unit tests
- `platform.ts` — simple but relied upon for keyboard shortcut display
- `theme.svelte.ts`, `updater.svelte.ts` — low risk but untested
- `tool-configs.ts` — only 2 tests for a 178-line file

### 22. `tool-configs.ts` color repetition

| Effort | Risk |
|--------|------|
| S | S |

The same Tailwind class strings (blue, amber, purple, emerald palettes) are repeated 20 times across tool config entries. Extract color constants to reduce repetition and make palette changes easier.

---

## What's in Good Shape

- **Store architecture** — clean, flat, well-tested (165 tests passing)
- **Svelte 5 runes** — consistent usage, correct immutable update patterns
- **Component organization** — clear feature-based grouping, no prop drilling
- **Test infrastructure** — replay harness with invariant checking is impressive
- **Routing** — minimal and appropriate for a desktop SPA
- **Config files** — clean, no bloat
- **Tailwind v4** — modern CSS-based config, no unnecessary customization
