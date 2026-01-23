# Caipi TODOs

> **Note:** Keep this file up to date. Remove entries once they have been implemented.

---

## High Priority

### Connect Agent Task List to UI
The Claude agent can create and manage tasks via `TaskCreate`, `TaskUpdate`, `TaskList`, and `TaskGet` tools. These tasks should be displayed in our right sidebar's task list component.

**Research needed:**
- Understand how task-related events are emitted by `claude-agent-sdk-rs`
- What data structure do tasks have (id, subject, description, status, owner, blocks/blockedBy)?
- How do we receive task creation, updates, and completion events?

**Implementation:**
- Listen for task-related events from the SDK
- Store task state in the chat store (or a dedicated task store)
- Display tasks in the `TaskList.svelte` component in the right sidebar
- Show task status (pending, in_progress, completed)
- Update UI in real-time as the agent creates/updates/completes tasks

### Refactor ChatContainer.svelte (God Component)
ChatContainer.svelte has grown to ~540 lines with too many responsibilities:
- Event listening and processing (`handleClaudeEvent` switch statement)
- Permission handling logic
- Message queue management
- Keyboard shortcut handling
- UI rendering

Every new feature touches this file, increasing risk of bugs and merge conflicts.

**Refactor to:**
- Extract event handling into a separate module/service
- Extract keyboard shortcut handling
- Keep ChatContainer focused on layout and composition
- Target: ~200-300 lines

---

## Medium Priority

### Duplicated Activity Matching Logic
"Find activity by toolUseId, fall back to tool type" appears in 2-3 places.

**Fix:**
- Extract into a utility function: `findActivityForEvent(activities, toolUseId, toolType)`

### Rename "Recent Projects" to "Recent Folders"
The folder picker shows "Recent projects" but these are really just folders.

**Requirements:**
- Update label from "Recent projects" to "Recent folders"
- Check for any other references to "projects" that should be "folders"

### Show Context Usage Percentage
The token count below the input bar isn't very meaningful to users.

**Requirements:**
- Replace raw token count with percentage of context window used
- Don't show exact token numbers, just the percentage
- SDK should provide this information (or we calculate from known context window size)
- Consider a visual indicator (progress bar or percentage text)

### Timer Below Input Not Working
The timer display below the input bar doesn't show meaningful timing information.

**Requirements:**
- Timer should run while Claude is actively processing
- Timer should pause when waiting for user input (to keep timing fair)
- Display the cumulative time Claude spent processing for the current turn
- Reset when a new user message is sent
- Display in a clear format (e.g., "2m 34s")

### Tool Spinner Only During Execution
The activity card spinner currently appears as soon as a tool is requested, but it should only spin when the tool is actually executing.

**Current behavior:**
- Spinner shows during the "prompting" phase (tool requested but not yet running)

**Desired behavior:**
- No spinner while tool is being prepared/prompted
- Spinner only appears when the tool is actively executing
- This gives users accurate feedback about what's actually happening

### Excessive Whitespace in Responses
Agent responses have too much vertical spacing between paragraphs.

**Requirements:**
- Review CSS for message content rendering
- Adjust paragraph margins/spacing
- Ensure consistent typography with reasonable line height

---

## Completed

- ~~Skills Support~~ (enabled skill discovery via `setting_sources` in SDK, skills require permission before loading, tracked in right sidebar after user approval, styled to match task list)
- ~~Single Source of Truth for App State~~ (implemented event-driven pattern: backend emits `StateChanged` event after updating permission mode/model, frontend syncs via `appStore.syncState()`)
- ~~Extract Permission Hook Logic (agent.rs)~~ (refactored 180-line hook to ~40 lines using 7 helper functions: `allow_response`, `deny_response`, `check_abort_decision`, `extract_tool_info`, `check_mode_decision`, `requires_permission`, `build_permission_description`, `prompt_user_permission`)
- ~~Split chat.ts Store~~ (split into messageStore, activityStore, permissionStore with chat.ts as coordinator)
- ~~Permission Modes UI~~ (3 modes: Default, Edit, Danger)
- ~~Model Switching~~ (cycling button with dot sizes)
- ~~Stop Button Styling~~ (consistent with send button)
- ~~Light/Dark Mode Toggle~~ (sun/moon in header)
- ~~App Icon Replacement~~ (Caipi glass icon)
- ~~Logo Watermark Adjustments~~ (opacity and size)
- ~~Send Button Styling~~ (white icon, square shape)
- ~~Remove "Shift+Enter for New Line" Hint~~
- ~~Parallel permissions bug~~ (multiple pending permissions support)
- ~~Fix Session Store Lock Held Across Await~~ (clone session, release lock before async work)
- ~~Stop Button Context Preservation~~ (drain stream after interrupt instead of breaking immediately)
- ~~Fix Store Subscription Memory Leaks~~ (migrated to Svelte 5 `$derived` pattern with `$store` syntax)
- ~~Migrate to Proper Svelte 5 Runes~~ (all components now use `$derived($store.property)`)
- ~~Add Unit Test Infrastructure~~ (Vitest + testing-library for frontend, Rust test modules for backend)
- ~~Unify Event Schema Between Rust and Frontend~~ (extended ChatEvent enum with missing fields, replaced all ad-hoc JSON emissions with typed variants)

---

## Notes

- Window resizing works well
- Main page layout looks good
- Sidebar toggles work correctly
- Folder picker and recent folders work as expected
- Permission dialog UI works correctly
- Plan mode removed (too complex to implement reliably from outside Claude Code's internals)
- Permission modes (Default, Edit, Danger) all work correctly with proper restrictions
- Light/dark mode switcher works
- Right sidebar (context panel) works correctly
- Message context persists correctly across the conversation
