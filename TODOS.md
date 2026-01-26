# Caipi TODOs

## Priority Legend
- **P0**: Critical - Broken functionality
- **P1**: High - Significant UX issues
- **P2**: Medium - Polish and improvements
- **P3**: Low - Nice to have

---

## Tool Use & Streaming Issues

### P1: Small vertical UI jump when agent completes
Tiny jump in chat history when the agent finishes responding and spinner disappears.

### P1: Properly text stream the final result from the agent
The final response from the agent should be streamed character-by-character instead of appearing all at once.

### P2: Add thinking UI element when agent is reasoning
Show a visual indicator when the agent is in its thinking/reasoning phase before responding.

### P2: Bundle multiple tool uses into single UI element
When many tool calls happen in sequence, bundle them into a single collapsible element instead of showing each one separately. Add a smooth vertical scroll animation that cycles through the tool call names.

**File**: `src/lib/components/chat/ActivityCard.svelte`

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

---

## File Explorer

### P2: Fix file alignment problem
Files in the file explorer don't align horizontally. Need to investigate and fix the layout so file names line up properly.

**File**: `src/lib/components/sidebar/FileExplorer.svelte` or `FileTreeItem.svelte`
