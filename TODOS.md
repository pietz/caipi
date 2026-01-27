# Caipi TODOs

## Priority Legend
- **P0**: Critical - Broken functionality
- **P1**: High - Significant UX issues
- **P2**: Medium - Polish and improvements
- **P3**: Low - Nice to have

---

## Investigation Needed

### P1: Tool execution check marks inconsistent
Some tool calls get check marks, others don't. Need to investigate why bash commands sometimes don't show completion state.

**Context**: Tool status is tracked in `src/lib/stores/chat.svelte.ts`. Events come from Rust backend via `claude:tool_start` and `claude:tool_end`. May be a race condition or missing event emission.

---

## UI Polish

### P2: Add thinking UI element when agent is reasoning
Show a visual indicator when the agent is in its thinking/reasoning phase before responding.

**Context**: Claude has extended thinking mode (enabled via Brain icon in footer). When active, there's a delay before response while model reasons. Should show a "Thinking..." indicator during this phase.

### P2: Add colors to permission mode indicators
- **Default mode**: Blue (shield icon + text)
- **Edit mode**: Purple (pencil icon + text)
- **Danger mode**: Already has red color effect

**File**: `src/lib/components/chat/MessageInput.svelte`

### P2: Reduce input height and limit growth
- Reduce vertical padding slightly (currently more padding at bottom than top with 2 lines)
- Limit textarea growth to maximum 4 rows
- After 4 rows, enable scrolling within the textarea

**File**: `src/lib/components/chat/MessageInput.svelte`

### P2: Fix file alignment problem
Files in the file explorer don't align horizontally. Need to investigate and fix the layout so file names line up properly.

**File**: `src/lib/components/sidebar/FileExplorer.svelte` or `FileTreeItem.svelte`

---

## Known Issues

### P3: Unused Rust code
`extract_tool_target` function in `tool_utils.rs` and `AgentError::Session` variant are defined but not yet used. Clean up or implement.

### P2: Email skill execution issues
User encountered issues with email skill. May be skill-specific or related to general command execution. Needs investigation to determine if it's a Caipi issue or skill configuration issue.

**Context**: Skills are Claude Code features that provide specialized capabilities. The email skill uses AppleScript to interact with Mail.app. Issue may be permission-related or skill implementation.

### P2: Verbose builder pattern in agent.rs
The 4-way match for `ClaudeAgentOptions` builder (lines 203-252) is repetitive. Refactor to chained conditionals.

**File**: `src-tauri/src/claude/agent.rs`

### P3: Hardcoded version in SessionPicker
Version `v0.1.0` is hardcoded at line 197. Should pull from config or constant.

**File**: `src/lib/components/folder/SessionPicker.svelte`

### P3: Effect without cleanup in SessionPicker
`loadSessions()` runs in `$effect` without cancellation. Could cause race conditions on fast remount.

**File**: `src/lib/components/folder/SessionPicker.svelte`

### P3: Remove orphaned FolderPicker if unused
`FolderPicker.svelte` may be fully replaced by `SessionPicker.svelte`. Verify and delete if unused.

**File**: `src/lib/components/folder/FolderPicker.svelte`
