# Caipi TODOs

## Priority Legend
- **P0**: Critical - Broken functionality
- **P1**: High - Significant UX issues
- **P2**: Medium - Polish and improvements
- **P3**: Low - Nice to have

---

## Tool Use & Streaming Issues

### P0: Tool spinner timing is broken
The spinner for tool calls stays active too long and doesn't stop when the tool actually completes. Symptoms:
- Text of following messages appears before spinner disappears
- Check marks sometimes missing (especially on bash commands)
- First tool call in a response wave behaves differently than subsequent ones
- Multiple sequential tool calls show spinners appearing one after another but finishing together

**Files to investigate**: `src/lib/stores/chat.svelte.ts`, `src/lib/utils/events.ts`, `src-tauri/src/claude/agent.rs`

### P0: Queued message appears in wrong position
When sending a message while the agent is still working:
- Message appears directly after the initial user message (wrong)
- Should stay at the very bottom until the current response finishes
- The agent should write into a message that's not at the bottom of the list

**Files to investigate**: `src/lib/stores/chat.svelte.ts`, `src/lib/components/chat/ChatContainer.svelte`

### P1: Small vertical UI jump when agent completes
Tiny jump in chat history when the agent finishes responding and spinner disappears.

---

## Input Footer Layout

### P2: Reposition context indicator to the right
Move the context window percentage indicator to the right side of the footer. Keep model selector and permission mode on the left.

**File**: `src/lib/components/chat/MessageInput.svelte`

### P2: Dynamic context indicator color
Change the context indicator circle color dynamically based on percentage:
- 0% = Rich green
- 50% = Yellow
- 100% = Deep red
Use a gradient interpolation between these colors.

**File**: `src/lib/components/ui/ContextIndicator.svelte`

---

## Model Switcher

### P2: Replace model size circle with Lucide target icon
Current circle causes text jumping. Use Lucide `target` icon instead:
- Small model (Haiku): Inner ring only
- Medium model (Sonnet): Two inner rings
- Large model (Opus): All three rings

Ensure icon has fixed width so text doesn't jump when switching models.

**Files**: `src/lib/components/chat/MessageInput.svelte`, potentially new `ModelIcon.svelte`

---

## Permission Mode Colors

### P2: Add colors to permission mode indicators
- **Default mode**: Blue (shield icon + text)
- **Edit mode**: Purple (pencil icon + text)
- **Danger mode**: Already has red color effect

Don't change entire UI, just the icon and text colors.

**File**: `src/lib/components/chat/MessageInput.svelte`

---

## Input Box

### P2: Reduce input height and limit growth
- Reduce vertical padding slightly (currently more padding at bottom than top with 2 lines)
- Limit textarea growth to maximum 4 rows
- After 4 rows, enable scrolling within the textarea

**File**: `src/lib/components/chat/MessageInput.svelte`

---

## Investigation Needed

### P1: Tool execution check marks inconsistent
Some tool calls get check marks, others don't. Need to investigate why bash commands sometimes don't show completion state.

### P2: Email skill execution issues
User encountered issues with email skill. May be skill-specific or related to general command execution. Needs investigation to determine if it's a Caipi issue or skill configuration issue.
