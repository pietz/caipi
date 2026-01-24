# Caipi Architectural Review

## P1 — Critical Bugs & Architecture Issues

### 1. Model switching doesn't take effect after first message *(Bug)*
**Files:** `src-tauri/src/claude/agent.rs:193-209`

The `ClaudeAgentOptions` is built with the current model, but only used when creating a new client:
```rust
let options = ClaudeAgentOptions::builder()
    .model(model_id)  // Built with current model
    .build();

if client_guard.is_none() {
    *client_guard = Some(ClaudeClient::new(options));  // Only used here
}
```
Once the client exists, `options` is discarded. The UI shows model changes but they never apply.

**Fix:** Either recreate client on model change, or use SDK method to change model on existing client.

---

### 2. No session cleanup — unbounded memory growth *(Bug)*
**Files:** `src-tauri/src/commands/chat.rs:69-77`

`create_session` inserts into `SessionStore` but there's no `destroy_session` command. Switching projects accumulates Claude clients and message history indefinitely.

**Fix:** Add `destroy_session` command and call it when switching folders.

---

### 3. Chat store over-engineering — facade + 4 micro-stores
**Files:** `src/lib/stores/chat.ts:87-96`, `messageStore.ts`, `activityStore.ts`, `permissionStore.ts`

The facade layer combines 4 stores into `combinedStore`, then re-wraps with method delegations:
```
messageStore + activityStore + permissionStore + metadataStore
    → combinedStore (derived)
    → chatStore (facade with delegation)
    → 7 exported derived stores
```

This creates maximum indirection with minimum benefit. `activityStore` (56 lines) and `permissionStore` (58 lines) are trivial wrappers around an array and object.

**Fix:** Either flatten into single store, or remove facade and use stores directly.

---

## P2 — Significant Issues

### 4. Stringly-typed events — runtime validation instead of compile-time
**Files:** `src/lib/stores/chat/streamCoordinator.ts:17-31`

The frontend uses a loose interface:
```typescript
interface ChatEvent {
  type: string;  // Not discriminated
  content?: string;
  activity?: ToolActivity;
  // ... many optional fields
}
```

While Rust has a proper `ChatEvent` enum. This pushes validation to runtime and makes frontend drift-prone vs backend.

**Fix:** Create discriminated union type matching Rust enum.

---

### 5. Session start flow duplicated in 3 places
**Files:** `src/routes/+page.svelte:45-57`, `SetupWizard.svelte`, `FolderPicker.svelte`

Each screen duplicates the same steps:
1. Validate folder
2. Update appStore
3. Read settings via `get(appStore)`
4. Invoke `create_session`
5. Set sessionId
6. Navigate

**Fix:** Extract to shared `startSession(folder)` function.

---

### 6. Theme subscription leaks in SetupWizard and FolderPicker *(Bug)*
**Files:** `src/lib/components/onboarding/SetupWizard.svelte:27-29`, `src/lib/components/folder/FolderPicker.svelte:31-33`

Both components call `.subscribe()` without cleanup:
```typescript
resolvedTheme.subscribe((theme) => {
  currentTheme = theme;
});
```

If screens are re-entered, subscriptions accumulate and cause memory leaks.

**Fix:** Either use `$derived()` (Svelte 5 pattern) or store unsubscribe and call in `onDestroy`.

---

### 7. Dead Rust code — events.rs module unused
**Files:** `src-tauri/src/commands/events.rs`

The module provides `emit_*` helpers (81 lines) but nothing calls them. Events are emitted directly in `chat.rs` and `hooks.rs`.

**Fix:** Delete the module.

---

## P3 — Minor Issues & Cleanup

### 8. SkillsList subscription leak
**Files:** `src/lib/components/sidebar/SkillsList.svelte:5-9`

Same pattern as P2-6 — manual subscription without cleanup.

**Fix:** Use `$derived()`.

---

### 9. Stub code in SetupWizard — suggestDesktopFolder()
**Files:** `src/lib/components/onboarding/SetupWizard.svelte:53-57`

```typescript
function suggestDesktopFolder() {
  const homeDir = '/Users/' + (typeof window !== 'undefined' ? '' : '');
  // For now, just leave it empty
}
```

Does nothing but is called on mount.

**Fix:** Remove or implement.

---

### 10. Incomplete drag-drop state in FolderPicker
**Files:** `src/lib/components/folder/FolderPicker.svelte:21-28`

```typescript
let dragOver = $state(false);
let dropZoneHover = $state(false);
```

Drag-drop errors out at runtime. State exists but feature is broken.

**Fix:** Either implement properly or remove the partial UI state.

---

### 11. Unused tokenCount/sessionDuration — never updated
**Files:** `src/lib/stores/chat.ts:18-31`, `MessageInput.svelte`

The metadata store maintains these values, MessageInput renders them, but no code updates them from backend events.

**Fix:** Either wire up backend events or remove the display/state.

---

### 12. Newline-buffered streaming causes perceived lag
**Files:** `src/lib/stores/chat/streamCoordinator.ts:90-103`

```typescript
lineBuffer += event.content;
const lines = lineBuffer.split('\n');
lineBuffer = lines.pop() || '';
// Only emit complete lines
```

Long lines without newlines appear "stuck" until a newline arrives.

**Fix:** Consider character-based streaming, or smaller chunk flushing.

---

### 13. String-based config in Rust backend
**Files:** `src-tauri/src/claude/agent.rs`, `src-tauri/src/claude/hooks.rs`

`permission_mode` and `model` are stored as `String` with match statements scattered throughout. No compile-time validation.

**Fix:** Use enums with compile-time validation.

---

### 14. Coordinator files misplaced in /stores/chat/

`streamCoordinator.ts` and `permissionCoordinator.ts` are domain logic, not stores. Their placement confuses what's state vs. behavior.

**Fix:** Move to `/lib/logic/` or `/lib/handlers/`.

---

## Summary Table

| Priority | Issue | Type |
|----------|-------|------|
| **P1** | Model switching doesn't apply after first message | Bug |
| **P1** | No session cleanup — memory leak | Bug |
| **P1** | Chat store facade over-engineering | Architecture |
| **P2** | Stringly-typed frontend events | Type Safety |
| **P2** | Session start flow duplicated 3x | Duplication |
| **P2** | Theme subscription leaks | Bug |
| **P2** | Unused events.rs module | Dead Code |
| **P3** | SkillsList subscription leak | Bug |
| **P3** | Stub suggestDesktopFolder() | Dead Code |
| **P3** | Incomplete drag-drop state | Dead Code |
| **P3** | Unused token/duration stats | Dead Code |
| **P3** | Newline buffering UX lag | UX |
| **P3** | String-based Rust config | Type Safety |
| **P3** | Coordinator file placement | Organization |

---

## Recommended Attack Order

1. **P1 bugs first** — Model switching and session cleanup are functional issues users will hit
2. **P2 subscription leaks** — Quick wins, prevent memory issues
3. **P1 store architecture** — Biggest complexity reduction
4. **P2 dead code** — events.rs deletion
5. **P2 session flow duplication** — Consolidate
6. **P3 cleanup** — Stub code, unused state, type improvements
