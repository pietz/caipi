# Caipi TODOs

> **Note:** Keep this file up to date. Remove entries once they have been implemented.

---

## High Priority

### 1. Permission System Not Working
The permission request flow is broken. When Claude tries to perform write operations (like creating a folder), it hits a security restriction but the UI doesn't handle the permission request properly - the user can't approve or deny the action.

### 2. Message Ordering Incorrect
Messages and tool calls are not displayed in chronological order. Tool calls appear to be sorted/grouped above text responses instead of respecting the actual sequence.

**Expected behavior:** If Claude sends "I'll create a folder", then executes a tool, then says "I see a security restriction" - these should appear in that exact order.

**Current behavior:** Tool calls seem to bubble up, and text responses accumulate below them.

### 3. Welcome Screen Shows Every Startup
The onboarding/welcome screen with CLI detection (installed + authenticated checks) appears on every app launch. The authentication check is particularly slow.

**Expected behavior:** Show welcome screen only on first launch. Skip it on subsequent launches since CLI status rarely changes.

---

## Medium Priority

### 4. Permission Modes UI
Claude Code has different operation modes that need to be exposed in the UI:
- Ask before every edit (default)
- Accept all edits
- Plan mode
- Dangerously accept all tool calls

**Requirements:**
- Add a button underneath the input bar to cycle through modes
- Display current mode visually
- Consider color-coding or clear labeling

### 5. Send Button Styling
Two issues with the send button:
- Icon color is dark gray on medium gray background - should be **white** for better contrast
- Button is not square (taller than wide) - should be equal dimensions

### 6. Input Bar Helper Text Readability
The text underneath the input bar (keyboard hints, etc.) is too light/hard to read against the background. Needs a **darker gray** color for better readability.

### 7. App Icon Replacement
Currently using the default Tauri/framework logo in the macOS dock.

**Requirements:**
- Use the Caipi glass icon (without circle background)
- Place in rounded square shape appropriate for macOS
- Research macOS icon requirements - needs specific transparent padding for menu bar icons
- May need multiple sizes/formats

---

## Low Priority

### 8. Logo Watermark Adjustments
The Caipi logo watermark in the empty chat background:
- Increase opacity slightly (currently too faint)
- Make it a bit larger (currently quite small)

### 9. Light/Dark Mode Toggle
Add a toggle in the top-right corner of the UI (possibly next to version number):
- Sun icon for light mode
- Moon icon for dark mode
- Clicking toggles between modes

### 10. Settings Pane
No settings UI exists yet. Need to design and implement a settings panel. (Future feature - scope TBD)

---

## Notes

- Window resizing works well
- Main page layout looks good
- Sidebar toggles work correctly
- Folder picker and recent projects work as expected
