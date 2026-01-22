# Caipi TODOs

> **Note:** Keep this file up to date. Remove entries once they have been implemented.

---

## High Priority

(No high priority items remaining)

---

## Medium Priority

### 2. Permission Modes UI
Claude Code has different operation modes that need to be exposed in the UI:
- Ask before every edit (default)
- Accept all edits
- Plan mode
- Dangerously accept all tool calls

**Requirements:**
- Add a button underneath the input bar to cycle through modes
- Display current mode visually
- Consider color-coding or clear labeling

### 3. Send Button Styling
Two issues with the send button:
- Icon color is dark gray on medium gray background - should be **white** for better contrast
- Button is not square (taller than wide) - should be equal dimensions

### 4. Input Bar Helper Text Readability
The text underneath the input bar (keyboard hints, etc.) is too light/hard to read against the background. Needs a **darker gray** color for better readability.

### 5. App Icon Replacement
Currently using the default Tauri/framework logo in the macOS dock.

**Requirements:**
- Use the Caipi glass icon (without circle background)
- Place in rounded square shape appropriate for macOS
- Research macOS icon requirements - needs specific transparent padding for menu bar icons
- May need multiple sizes/formats

---

## Low Priority

### 6. Logo Watermark Adjustments
The Caipi logo watermark in the empty chat background:
- Increase opacity slightly (currently too faint)
- Make it a bit larger (currently quite small)

### 7. Light/Dark Mode Toggle
Add a toggle in the top-right corner of the UI (possibly next to version number):
- Sun icon for light mode
- Moon icon for dark mode
- Clicking toggles between modes

### 8. Settings Pane
No settings UI exists yet. Need to design and implement a settings panel. (Future feature - scope TBD)

---

## Notes

- Window resizing works well
- Main page layout looks good
- Sidebar toggles work correctly
- Folder picker and recent projects work as expected
