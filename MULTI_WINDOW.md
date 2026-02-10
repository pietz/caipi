# Multi-Window / Multi-Session Feature

Design document for running multiple concurrent chat sessions in Caipi.

## Current Architecture (Single Session)

```
User Input → Tauri Command → Rust BackendSession → CLI Process → stdout JSON stream
                                                                       ↓
                                                              emit_chat_event()
                                                                       ↓
                                                          app_handle.emit("chat:event")  ← broadcasts to ALL windows
                                                                       ↓
                                                          Frontend listen("chat:event")
                                                                       ↓
                                                          shouldIgnoreEvent() filters by session_id
                                                                       ↓
                                                          Singleton chat store updates UI
```

**Key components:**
- `SessionStore` — `HashMap<String, Arc<dyn BackendSession>>` (already supports multiple sessions by ID)
- `emit_chat_event()` in `runtime.rs` — uses `app_handle.emit()` (global broadcast)
- `ChatEventEnvelope` — wraps events with `session_id` and `turn_id` metadata
- `shouldIgnoreEvent()` in `events.ts` — client-side filter comparing `event.sessionId` to `app.sessionId`
- Singleton stores — `app`, `chat`, `files` are module-level singletons
- Module-level globals — `lineBuffer` and `flushTimer` in `events.ts`
- Event listener — global `listen<ChatEvent>('chat:event', ...)` in `ChatContainer.svelte`

---

## Approach A: Multi-Window (Recommended)

Each session runs in its own native OS window. Tauri's `emit_to()` routes events directly to the correct window. Each window has an isolated JavaScript runtime, so all stores and globals are naturally separate.

### Why This Works

Each Tauri webview window is a **separate JS execution context**. This means:
- The singleton `app`, `chat`, `files` stores get their own instance per window — no refactoring needed
- The module-level `lineBuffer` and `flushTimer` are naturally isolated — no per-session maps needed
- `localStorage` is shared across windows (same origin), so persisted settings sync automatically

### Architecture

```
Window A (session-abc)                          Window B (session-xyz)
┌─────────────────────┐                        ┌─────────────────────┐
│ Own JS runtime       │                        │ Own JS runtime       │
│ Own app/chat/files   │                        │ Own app/chat/files   │
│ Own lineBuffer       │                        │ Own lineBuffer       │
│                      │                        │                      │
│ getCurrent().listen()│                        │ getCurrent().listen()│
│ "chat:event"         │                        │ "chat:event"         │
└────────▲─────────────┘                        └────────▲─────────────┘
         │ emit_to("session-abc")                        │ emit_to("session-xyz")
         │                                               │
┌────────┴───────────────────────────────────────────────┴─────────────┐
│                        Rust Backend                                   │
│                                                                       │
│  SessionStore: HashMap<String, Arc<dyn BackendSession>>               │
│  WindowRegistry: HashMap<String, String>  (session_id → window_label) │
│                                                                       │
│  emit_chat_event() → looks up window_label → emit_to(label, ...)      │
└───────────────────────────────────────────────────────────────────────┘
```

### Changes Required

#### Rust Backend

**1. Window registry** (new state)

```rust
// New type: maps session_id → window_label
pub type WindowRegistry = Arc<Mutex<HashMap<String, String>>>;
```

Register in Tauri builder via `.manage(WindowRegistry::default())`.

**2. `emit_chat_event()` — targeted emission** (`runtime.rs`)

```rust
pub fn emit_chat_event(
    app_handle: &AppHandle,
    session_id: Option<&str>,
    turn_id: Option<&str>,
    event: &ChatEvent,
) {
    let payload = ChatEventEnvelope { session_id, turn_id, event };

    // Try targeted emit first, fall back to broadcast
    if let Some(sid) = session_id {
        if let Some(registry) = app_handle.try_state::<WindowRegistry>() {
            if let Ok(map) = registry.lock() {
                if let Some(label) = map.get(sid) {
                    let _ = app_handle.emit_to(label, CHAT_EVENT_CHANNEL, &payload);
                    return;
                }
            }
        }
    }
    // Fallback: broadcast (backwards-compatible)
    let _ = app_handle.emit(CHAT_EVENT_CHANNEL, &payload);
}
```

**3. `create_session` command — register window mapping**

When a session is created, the frontend passes its window label. The backend registers the `session_id → window_label` mapping.

**4. `create_window` command** (new)

```rust
#[tauri::command]
async fn create_window(app: tauri::AppHandle) -> Result<String, String> {
    let label = format!("chat-{}", uuid::Uuid::new_v4());
    WebviewWindowBuilder::new(&app, &label, WebviewUrl::App("index.html".into()))
        .title("Caipi")
        .inner_size(900.0, 640.0)
        .hidden_title(true)
        .title_bar_style(TitleBarStyle::Overlay)
        // ... match existing window config
        .build()
        .map_err(|e| e.to_string())?;
    Ok(label)
}
```

**5. Window close cleanup**

Listen for window close events to clean up sessions and deregister from the window registry.

#### Frontend

**1. Event listener — scoped to current window** (`ChatContainer.svelte`)

```typescript
// Before (broadcasts to all windows):
unlisten = await listen<ChatEvent>('chat:event', handler);

// After (only receives events for THIS window):
import { getCurrent } from '@tauri-apps/api/webviewWindow';
unlisten = await getCurrent().listen<ChatEvent>('chat:event', handler);
```

**2. Session creation — pass window label**

```typescript
import { getCurrent } from '@tauri-apps/api/webviewWindow';
const windowLabel = getCurrent().label;
// Pass windowLabel to create_session so backend can register the mapping
```

**3. "New Window" action**

A menu item, keyboard shortcut (Cmd+N), or button that calls the `create_window` command. The new window loads the same SvelteKit app and goes through the normal startup flow (folder picker → session creation).

#### Capabilities (`default.json`)

```json
{
  "windows": ["main", "chat-*"],
  "permissions": [
    "core:webview:allow-create-webview-window",
    // ... existing permissions
  ]
}
```

### Shared State Across Windows

Some state genuinely needs to sync across windows:

| State | Mechanism | Notes |
|-------|-----------|-------|
| Settings (model, permissions, theme) | `localStorage` (shared origin) | Each window reads on startup; changes auto-visible |
| License status | `localStorage` or Rust managed state | Check on window creation |
| Default backend / CLI paths | `localStorage` | Per-window override possible |
| "Which sessions are open?" | Rust `WindowRegistry` | Query via Tauri command |

For real-time sync (e.g., changing theme in one window updates all), use `app_handle.emit()` (broadcast) for a dedicated `"settings:changed"` event channel — separate from the session-scoped chat events.

### Window Lifecycle

| Event | Behavior |
|-------|----------|
| New Window | Spawns webview, goes through folder picker or opens with a specified folder |
| Window Close | Destroys session, removes from WindowRegistry, cleans up CLI process |
| Last Window Close | App exits (default macOS/Windows behavior) |
| Session ends (Complete) | Window stays open, user can start new session or close |
| App quit (Cmd+Q) | All windows close, all sessions cleaned up |

### Advantages

- **Zero store refactoring** — isolated JS runtimes per window
- **Zero event multiplexing** — `emit_to` handles routing in Rust
- **Existing UI unchanged** — no tab bar, no component parameterization
- **OS-level window management** — snap, tile, multi-monitor, full-screen
- **Small Rust diff** — window registry + `emit` → `emit_to` + new command
- **Incremental** — can ship with just "New Window" and iterate

### Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Memory per webview (~50-100MB each) | Acceptable for desktop; most users will have 2-4 windows |
| Settings drift between windows | Use `localStorage` + broadcast event for real-time sync |
| Window state not persisted on restart | Save open windows/folders to localStorage; restore on next launch |
| Windows deadlock on Windows OS | Always use async commands for window creation (Tauri docs warning) |

---

## Approach B: Tab-Based (Single Window)

A single Tauri window with a tab bar. The frontend manages multiple sessions by multiplexing events.

### Architecture

```
┌──────────────────────────────────────────────────┐
│                 Single Window                      │
│  ┌──────────┬──────────┬──────────┐               │
│  │ Tab A    │ Tab B    │ Tab C    │  ← tab bar    │
│  └──────────┴──────────┴──────────┘               │
│  ┌────────────────────────────────────────────┐   │
│  │            Active Tab Content               │   │
│  │     (renders selected session's chat)       │   │
│  └────────────────────────────────────────────┘   │
│                                                    │
│  SessionManager: Map<sessionId, {                  │
│    chat: ChatState,                                │
│    files: FilesState,                              │
│    lineBuffer: string,                             │
│    flushTimer: number                              │
│  }>                                                │
│                                                    │
│  Event dispatcher:                                 │
│    listen("chat:event") → route by sessionId       │
│    → update correct ChatState                      │
└──────────────────────────────────────────────────┘
```

### Changes Required

#### Frontend (Major Refactoring)

**1. Per-session store instances**

The singleton `chat` and `files` stores must become a factory or class instantiated per session:

```typescript
class SessionManager {
  sessions = $state<Map<string, SessionContext>>(new Map());
  activeSessionId = $state<string | null>(null);

  get activeSession(): SessionContext | undefined {
    return this.activeSessionId ? this.sessions.get(this.activeSessionId) : undefined;
  }
}

interface SessionContext {
  chat: ChatState;
  files: FilesState;
  lineBuffer: string;
  flushTimer: ReturnType<typeof setTimeout> | null;
}
```

**2. Event dispatcher**

Replace the direct `handleClaudeEvent` with a router that looks up the correct `SessionContext`:

```typescript
listen<ChatEvent>('chat:event', (event) => {
  const sessionId = event.payload.sessionId;
  const ctx = sessionManager.sessions.get(sessionId);
  if (!ctx) return;
  handleClaudeEvent(event.payload, ctx.chat, ctx.lineBuffer, ctx.flushTimer, ...);
});
```

**3. Component parameterization**

`ChatContainer`, `MessageList`, and related components must accept a `ChatState` prop instead of importing the global singleton.

**4. Tab bar component** (new)

New UI component for switching between sessions, showing session names, close buttons, drag-to-reorder.

**5. Module globals elimination**

`lineBuffer`, `flushTimer`, `onContentChange` in `events.ts` must move into `SessionContext`.

#### Rust Backend

Minimal changes — the broadcast `app_handle.emit()` still works since the frontend handles routing. Optionally keep `shouldIgnoreEvent()` as extra safety.

### Advantages

- **Single webview** — lower memory footprint
- **Easier inter-tab UX** — drag sessions, see all tabs at a glance
- **Simpler window lifecycle** — one window = one app
- **No Tauri multi-window API needed** — purely frontend work

### Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Large frontend refactoring | Break into phases: stores first, then UI, then polish |
| All events processed in single thread | `shouldIgnoreEvent` is fast; only active tab renders |
| Singleton store assumptions scattered through codebase | Audit all imports of `chat`/`files` singletons |
| `events.ts` globals are implicit coupling | Extract into `SessionContext` class |
| Background tabs accumulate invisible state | Limit to N tabs; show memory warnings |

---

## Comparison

| Dimension | Multi-Window (A) | Tabs (B) |
|-----------|-----------------|----------|
| **Store refactoring** | None | Major (singletons → per-session) |
| **Event routing** | `emit_to` in Rust (1 function change) | Frontend dispatcher (new layer) |
| **UI changes** | "New Window" button/shortcut only | Tab bar + component parameterization |
| **Rust changes** | Window registry + `emit_to` + new command | Minimal |
| **Frontend changes** | Scoped listener + window label passing | Extensive refactoring |
| **JS isolation** | Free (separate runtimes) | Manual (per-session state maps) |
| **Memory per session** | ~50-100MB (full webview) | ~5-10MB (JS state only) |
| **OS integration** | Native window management | None |
| **Side-by-side sessions** | Yes (OS window tiling) | No (unless split-view built) |
| **Implementation effort** | Small | Large |
| **Risk of regressions** | Low (existing code untouched) | High (store layer rewrite) |

---

## Recommendation

**Start with Approach A (Multi-Window).** It delivers multi-session with minimal code changes and zero risk to existing functionality. The Rust diff is small and contained, and the frontend changes are limited to two lines (scoped listener + window label passing) plus a "New Window" action.

Approach B (Tabs) could be layered on later if desired — the per-session store refactoring it requires is orthogonal and can be done independently. A future hybrid approach (tabs within windows, like VS Code) would build on both.

### Implementation Order for Approach A

1. Add `WindowRegistry` to Rust state, update `emit_chat_event` to use `emit_to`
2. Update `create_session` to accept and register `window_label`
3. Add `create_window` Tauri command
4. Update capabilities to allow window creation and scope to `chat-*` windows
5. Frontend: switch to `getCurrent().listen()` in ChatContainer
6. Frontend: pass window label during session creation
7. Add "New Window" shortcut (Cmd+N) and/or menu item
8. Handle window close → session cleanup
9. Add settings sync broadcast channel
10. Test concurrent sessions with targeted event delivery
