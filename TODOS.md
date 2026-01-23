# Caipi TODOs

> **Note:** Keep this file up to date. Remove entries once they have been implemented.

---

## High Priority

No high priority tasks at the moment.

---

## Medium Priority

### Permission Modes UI
Claude Code has different operation modes that need to be exposed in the UI:
- **Default mode:** Can read files, but cannot write or execute
- **Accept edits mode:** Can read and write, but cannot execute
- **Plan mode:** Can read, ends with a plan that needs approval/rejection
- **Dangerous mode:** Read, write, and execute all tools without confirmation

**Requirements:**
- Add a button underneath the input bar to cycle through modes
- Display current mode visually (icon or label)
- Consider color-coding for dangerous mode

### Model Switching
Users need to be able to switch between available Claude models.

**Requirements:**
- Add a button underneath the input bar showing current model
- Clicking cycles through: Opus 4.5 → Sonnet 4.5 → Haiku 4.5
- Persist selection across sessions (or per conversation)

### Stop Button Styling
The stop button appears on a transparent background, losing the visual consistency with the send button.

**Requirements:**
- Keep the same button styling as the send button
- Only swap the icon (send → stop)
- Maintain proper contrast and visibility

### Show Context Usage Percentage
The token count below the input bar isn't very meaningful to users.

**Requirements:**
- Replace raw token count with percentage of context window used
- SDK should provide this information
- Consider a visual indicator (progress bar or percentage text)

### Timer Below Input Not Working
The timer display below the input bar doesn't show meaningful timing information.

**Requirements:**
- Track time since conversation started or since last message
- Or track agent response time
- Display in a clear format (e.g., "2m 34s")

### Excessive Whitespace in Responses
Agent responses have too much vertical spacing between paragraphs, making them look sparse.

**Requirements:**
- Review CSS for message content rendering
- Adjust paragraph margins/spacing
- Ensure consistent typography with reasonable line height

### Send Button Styling
Two issues with the send button:
- Icon color is dark gray on medium gray background - should be **white** for better contrast
- Button may not be square (taller than wide) - should be equal dimensions

---

## Low Priority

### Remove "Shift+Enter for New Line" Hint
The hint text about Shift+Enter for new lines takes up space and can be assumed as standard behavior.

**Requirements:**
- Remove or hide this hint
- Consider showing it only on first use or in a tooltip

### App Icon Replacement
Currently using the default Tauri/framework logo in the macOS dock.

**Requirements:**
- Use the Caipi glass icon (without circle background)
- Place in rounded square shape appropriate for macOS
- Research macOS icon requirements - needs specific transparent padding for menu bar icons
- May need multiple sizes/formats

### Logo Watermark Adjustments
The Caipi logo watermark in the empty chat background:
- Increase opacity slightly (currently too faint)
- Make it a bit larger (currently quite small)

### Light/Dark Mode Toggle
Add a toggle in the top-right corner of the UI (possibly next to version number):
- Sun icon for light mode
- Moon icon for dark mode
- Clicking toggles between modes

### Settings Pane
No settings UI exists yet. Need to design and implement a settings panel. (Future feature - scope TBD)

---

## Notes

- Window resizing works well
- Main page layout looks good
- Sidebar toggles work correctly
- Folder picker and recent projects work as expected
- Permission dialog UI works correctly
