# Frontend Store Rebuild Plan

## The Problem

The current store architecture has 10 files totaling ~800 lines for what is conceptually simple state:
- App state (screen, folder, session, sidebars, settings)
- Chat state (messages, streaming, tools, permissions)
- File tree state

The complexity comes from:
1. **Micro-stores** - `activityStore` (56 lines) and `permissionStore` (58 lines) wrap single arrays/objects
2. **Facade pattern** - `chat.ts` combines 4 stores into `combinedStore`, then re-wraps with method delegations
3. **Svelte 4 patterns** - Using `writable`/`derived` when Svelte 5 runes are simpler
4. **Coordinator files** - Domain logic mixed into `/stores/` directory
5. **Derived store proliferation** - 15+ exported derived stores when `$derived()` in components suffices

## The Goal

Reduce to 2-3 simple rune-based stores:
```
src/lib/stores/
  app.svelte.ts    (~60 lines)  - app state + settings
  chat.svelte.ts   (~120 lines) - messages + streaming + activities + permissions
  files.svelte.ts  (~40 lines)  - file tree (optional, could merge into app)
```

Plus one utility file:
```
src/lib/utils/
  events.ts        (~80 lines)  - event handling logic (moved from coordinators)
```

**Target: ~300 lines total, down from ~800**

---

## Phase 1: Create New Stores (Non-Breaking)

Create new stores alongside existing ones. This lets us migrate components incrementally.

### Step 1.1: Create `app.svelte.ts`

```typescript
// src/lib/stores/app.svelte.ts
import { invoke } from '@tauri-apps/api/core';

export type Screen = 'loading' | 'onboarding' | 'folder' | 'chat';
export type PermissionMode = 'default' | 'acceptEdits' | 'bypassPermissions';
export type Model = 'opus' | 'sonnet' | 'haiku';

function getPersistedModel(): Model {
  if (typeof localStorage !== 'undefined') {
    const saved = localStorage.getItem('caipi:model');
    if (saved === 'opus' || saved === 'sonnet' || saved === 'haiku') return saved;
  }
  return 'sonnet';
}

class AppState {
  // Navigation
  screen = $state<Screen>('loading');
  loading = $state(true);
  error = $state<string | null>(null);

  // Session
  folder = $state<string | null>(null);
  sessionId = $state<string | null>(null);

  // UI
  leftSidebar = $state(false);
  rightSidebar = $state(false);

  // Settings
  permissionMode = $state<PermissionMode>('default');
  model = $state<Model>(getPersistedModel());

  // CLI status (for onboarding)
  cliInstalled = $state(false);
  cliVersion = $state<string | null>(null);

  // Derived
  folderName = $derived(this.folder?.split('/').pop() ?? '');

  // Methods
  setModel(model: Model) {
    this.model = model;
    localStorage?.setItem('caipi:model', model);
  }

  cycleModel() {
    const models: Model[] = ['opus', 'sonnet', 'haiku'];
    const next = (models.indexOf(this.model) + 1) % models.length;
    this.setModel(models[next]);
  }

  cyclePermissionMode() {
    const modes: PermissionMode[] = ['default', 'acceptEdits', 'bypassPermissions'];
    const next = (modes.indexOf(this.permissionMode) + 1) % modes.length;
    this.permissionMode = modes[next];
  }

  async startSession(folder: string): Promise<boolean> {
    try {
      this.folder = folder;
      this.sessionId = await invoke<string>('create_session', {
        folderPath: folder,
        permissionMode: this.permissionMode,
        model: this.model,
      });
      this.screen = 'chat';
      return true;
    } catch (e) {
      this.error = e instanceof Error ? e.message : 'Failed to start session';
      return false;
    }
  }

  reset() {
    this.screen = 'loading';
    this.loading = true;
    this.error = null;
    this.folder = null;
    this.sessionId = null;
    this.leftSidebar = false;
    this.rightSidebar = false;
  }
}

export const app = new AppState();
```

**Key simplifications:**
- Class-based state with `$state()` runes - no writable/derived boilerplate
- `startSession()` consolidates duplicated logic from 3 components
- No separate derived store exports - use `app.folderName` directly
- ~60 lines vs current 111 lines (and cleaner)

### Step 1.2: Create `chat.svelte.ts`

```typescript
// src/lib/stores/chat.svelte.ts

export interface Message {
  id: string;
  role: 'user' | 'assistant' | 'error';
  content: string;
  timestamp: number;
  activities?: ToolActivity[];
}

export interface ToolActivity {
  id: string;
  toolType: string;
  target: string;
  status: 'running' | 'completed' | 'error' | 'aborted';
  timestamp: number;
}

export interface PermissionRequest {
  id: string;
  activityId: string;
  tool: string;
  description: string;
}

export interface TodoItem {
  id: string;
  text: string;
  done: boolean;
  active: boolean;
}

class ChatState {
  // Messages (finalized)
  messages = $state<Message[]>([]);

  // Streaming state
  isStreaming = $state(false);
  streamingText = $state('');
  streamingActivities = $state<ToolActivity[]>([]);

  // Permissions (transient during streaming)
  pendingPermissions = $state<Map<string, PermissionRequest>>(new Map());

  // Metadata from backend
  todos = $state<TodoItem[]>([]);
  skills = $state<string[]>([]);

  // Message queue (for sending while streaming)
  queue = $state<string[]>([]);

  // Derived
  hasPermissions = $derived(this.pendingPermissions.size > 0);

  // --- Message methods ---

  addUserMessage(content: string) {
    this.messages.push({
      id: crypto.randomUUID(),
      role: 'user',
      content,
      timestamp: Date.now() / 1000,
    });
  }

  // --- Streaming methods ---

  startStreaming() {
    this.isStreaming = true;
    this.streamingText = '';
    this.streamingActivities = [];
  }

  appendText(text: string) {
    this.streamingText += text;
  }

  addActivity(activity: ToolActivity) {
    this.streamingActivities.push(activity);
  }

  updateActivityStatus(id: string, status: ToolActivity['status']) {
    const activity = this.streamingActivities.find(a => a.id === id);
    if (activity) activity.status = status;
  }

  finalize() {
    // Convert streaming state to finalized message
    if (this.streamingText || this.streamingActivities.length > 0) {
      this.messages.push({
        id: crypto.randomUUID(),
        role: 'assistant',
        content: this.streamingText,
        timestamp: Date.now() / 1000,
        activities: this.streamingActivities.length > 0
          ? [...this.streamingActivities]
          : undefined,
      });
    }

    this.isStreaming = false;
    this.streamingText = '';
    this.streamingActivities = [];
    this.pendingPermissions.clear();
  }

  // --- Permission methods ---

  addPermission(request: PermissionRequest) {
    this.pendingPermissions.set(request.activityId, request);
  }

  removePermission(activityId: string) {
    this.pendingPermissions.delete(activityId);
  }

  // --- Queue methods ---

  enqueue(message: string) {
    this.queue.push(message);
  }

  dequeue(): string | undefined {
    return this.queue.shift();
  }

  // --- Reset ---

  reset() {
    this.messages = [];
    this.isStreaming = false;
    this.streamingText = '';
    this.streamingActivities = [];
    this.pendingPermissions.clear();
    this.todos = [];
    this.skills = [];
    this.queue = [];
  }
}

export const chat = new ChatState();
```

**Key simplifications:**
- Single class instead of 4 micro-stores + facade
- Direct mutation with `$state()` - no spread operator dance
- ~120 lines vs current ~400 lines across messageStore + activityStore + permissionStore + chat.ts
- Removed `StreamItem` complexity - just use `streamingText` + `streamingActivities` arrays

### Step 1.3: Create `files.svelte.ts` (Optional)

```typescript
// src/lib/stores/files.svelte.ts

export interface FileEntry {
  name: string;
  type: 'file' | 'folder';
  path: string;
  children?: FileEntry[];
}

class FilesState {
  tree = $state<FileEntry[]>([]);
  expanded = $state<Set<string>>(new Set());
  selected = $state<string | null>(null);
  loading = $state(false);

  setTree(entries: FileEntry[]) {
    this.tree = entries;
    this.loading = false;
  }

  toggleExpanded(path: string) {
    if (this.expanded.has(path)) {
      this.expanded.delete(path);
    } else {
      this.expanded.add(path);
    }
  }

  reset() {
    this.tree = [];
    this.expanded.clear();
    this.selected = null;
    this.loading = false;
  }
}

export const files = new FilesState();
```

**~40 lines vs current 115 lines**

---

## Phase 2: Create Event Handler Utility

Move coordinator logic to a simple utility function. This isn't a store - it's just event handling.

```typescript
// src/lib/utils/events.ts
import { chat } from '$lib/stores/chat.svelte';
import { app } from '$lib/stores/app.svelte';

export interface ClaudeEvent {
  type: 'Text' | 'ToolStart' | 'ToolComplete' | 'PermissionRequest' |
        'Complete' | 'Error' | 'StateChanged' | 'TodoWrite' | 'SkillStart';
  content?: string;
  activity?: { id: string; toolType: string; target: string; timestamp: number };
  id?: string;
  status?: string;
  tool?: string;
  description?: string;
  message?: string;
  permissionMode?: string;
  model?: string;
  todos?: Array<{ id: string; text: string; done: boolean; active: boolean }>;
  skill?: string;
}

export function handleClaudeEvent(event: ClaudeEvent, callbacks?: { onComplete?: () => void }) {
  switch (event.type) {
    case 'Text':
      if (event.content) chat.appendText(event.content);
      break;

    case 'ToolStart':
      if (event.activity) {
        chat.addActivity({
          ...event.activity,
          status: 'running',
        });
      }
      break;

    case 'ToolComplete':
      if (event.id) {
        chat.updateActivityStatus(event.id, event.status === 'error' ? 'error' : 'completed');
        chat.removePermission(event.id);
      }
      break;

    case 'PermissionRequest':
      if (event.id && event.tool && event.description) {
        chat.addPermission({
          id: event.id,
          activityId: event.id,
          tool: event.tool,
          description: event.description,
        });
      }
      break;

    case 'Complete':
      chat.finalize();
      callbacks?.onComplete?.();
      break;

    case 'Error':
      chat.finalize();
      if (event.message) {
        chat.messages.push({
          id: crypto.randomUUID(),
          role: 'error',
          content: event.message,
          timestamp: Date.now() / 1000,
        });
      }
      break;

    case 'StateChanged':
      if (event.permissionMode) {
        app.permissionMode = event.permissionMode as any;
      }
      if (event.model) {
        app.model = event.model as any;
      }
      break;

    case 'TodoWrite':
      if (event.todos) {
        chat.todos = event.todos;
      }
      break;

    case 'SkillStart':
      if (event.skill && !chat.skills.includes(event.skill)) {
        chat.skills.push(event.skill);
      }
      break;
  }
}
```

**~80 lines - simple switch statement instead of coordinator class with callbacks**

---

## Phase 3: Migrate Components

### 3.1 Update ChatContainer.svelte

```diff
- import {
-   appStore,
-   chatStore,
-   createStreamCoordinator,
-   createPermissionCoordinator,
-   type ChatEvent,
- } from '$lib/stores';
+ import { app } from '$lib/stores/app.svelte';
+ import { chat } from '$lib/stores/chat.svelte';
+ import { handleClaudeEvent, type ClaudeEvent } from '$lib/utils/events';

- const sessionId = $derived($appStore.sessionId);
+ const sessionId = $derived(app.sessionId);

- const messages = $derived($chatStore.messages);
+ const messages = $derived(chat.messages);

- const streamCoordinator = createStreamCoordinator({
-   onComplete: processQueuedMessages,
- });

  onMount(async () => {
-   unlisten = await listen<ChatEvent>('claude:event', (event) => {
-     streamCoordinator.handleEvent(event.payload);
+   unlisten = await listen<ClaudeEvent>('claude:event', (event) => {
+     handleClaudeEvent(event.payload, { onComplete: processQueuedMessages });
      scrollToBottom();
    });
  });

  async function sendMessage(message: string) {
-   chatStore.addMessage({ ... });
-   chatStore.setStreaming(true);
+   chat.addUserMessage(message);
+   chat.startStreaming();
    // ...
  }
```

### 3.2 Update other components similarly

Each component that uses stores:
1. Replace `$appStore` with `app`
2. Replace `$chatStore` with `chat`
3. Replace `chatStore.method()` with `chat.method()`
4. Remove `$derived($store.prop)` - just use `prop` directly from the class

### 3.3 Consolidate session start logic

Remove duplicated session start from:
- `+page.svelte` (lines 45-57)
- `SetupWizard.svelte`
- `FolderPicker.svelte`

Replace with:
```typescript
await app.startSession(folderPath);
```

---

## Phase 4: Remove Old Code

Once all components are migrated:

### Delete these files:
```
src/lib/stores/
  app.ts                    (replaced by app.svelte.ts)
  chat.ts                   (replaced by chat.svelte.ts)
  messageStore.ts           (merged into chat.svelte.ts)
  activityStore.ts          (merged into chat.svelte.ts)
  permissionStore.ts        (merged into chat.svelte.ts)
  files.ts                  (replaced by files.svelte.ts)
  chat/
    index.ts                (delete directory)
    streamCoordinator.ts    (replaced by utils/events.ts)
    permissionCoordinator.ts (inline into ChatContainer)
```

### Update `src/lib/stores/index.ts`:
```typescript
export { app, type Screen, type PermissionMode, type Model } from './app.svelte';
export { chat, type Message, type ToolActivity, type PermissionRequest, type TodoItem } from './chat.svelte';
export { files, type FileEntry } from './files.svelte';
```

---

## Phase 5: Fix Subscription Leaks

With the new stores, components use `$derived()` automatically. But verify:

### Components to check:
- [ ] `SetupWizard.svelte` - remove manual theme subscription
- [ ] `FolderPicker.svelte` - remove manual theme subscription
- [ ] `SkillsList.svelte` - remove manual chatStore subscription

### Fix pattern:
```diff
- let currentTheme = $state<'light' | 'dark'>('dark');
- resolvedTheme.subscribe((theme) => {
-   currentTheme = theme;
- });
+ const currentTheme = $derived($resolvedTheme);
```

---

## Phase 6: Cleanup Dead Code

While we're here, also remove:

1. **Rust `events.rs`** - unused helper module
2. **`Welcome.svelte`** - verify if used, delete if not
3. **`suggestDesktopFolder()`** - stub function in SetupWizard
4. **Drag-drop state** in FolderPicker - incomplete feature
5. **`tokenCount`/`sessionDuration`** - never updated from backend

---

## Summary

### Before:
```
stores/
  app.ts                 111 lines
  chat.ts                175 lines
  messageStore.ts        226 lines
  activityStore.ts        56 lines
  permissionStore.ts      58 lines
  files.ts               115 lines
  theme.ts                83 lines  (keep as-is)
  chat/
    index.ts               4 lines
    streamCoordinator.ts 170 lines
    permissionCoordinator.ts 102 lines
                        ─────────
                        1,100 lines
```

### After:
```
stores/
  app.svelte.ts           60 lines
  chat.svelte.ts         120 lines
  files.svelte.ts         40 lines
  theme.ts                83 lines  (unchanged)
  index.ts                10 lines
                        ─────────
                         313 lines

utils/
  events.ts               80 lines
                        ─────────
Total:                   393 lines
```

**Reduction: ~700 lines (64% less code)**

---

## Migration Order

1. **Create new files** (non-breaking) - can coexist with old stores
2. **Migrate ChatContainer first** - it's the main consumer
3. **Migrate sidebar components** - FileExplorer, ContextPanel, SkillsList
4. **Migrate onboarding** - SetupWizard, FolderPicker
5. **Migrate +page.svelte** - screen routing
6. **Delete old store files**
7. **Clean up dead code**

Each step should result in a working app. Don't try to do it all at once.

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Breaking reactivity | Test each component after migration |
| Missing edge cases | Keep old stores until fully migrated, compare behavior |
| Theme store interaction | Leave theme.ts unchanged - it works fine |
| Backend event format changes | Create TypeScript types matching Rust ChatEvent enum |

---

## Optional: Further Simplifications

If this goes well, consider:

1. **Merge `files` into `app`** - it's only ~40 lines and tightly coupled to folder selection
2. **Inline permission handling** - the keyboard shortcuts could live in ChatContainer directly
3. **Remove message queue** - evaluate if queuing is actually needed or if we can just disable input during streaming
