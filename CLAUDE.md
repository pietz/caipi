# Caipi - Claude Code Instructions

## Overview

Caipi is a Tauri 2.0 desktop application that provides a friendly chat UI for Claude Code's agent capabilities. It wraps the Claude Code CLI with a native desktop interface.

## Tech Stack

- **Frontend**: Svelte 5 (with runes) + TypeScript + SvelteKit (static adapter)
- **Styling**: Tailwind CSS v3 + custom CSS variables
- **Backend**: Rust + Tauri 2.0
- **Claude Integration**: `claude-agent-sdk-rs` v0.6

## Commands

### Development
```bash
# Run the app in development mode
npm run tauri dev

# Type check the frontend
npm run check

# Build for production
npm run tauri build
```

### Rust Only
```bash
cd src-tauri
cargo check    # Check for errors
cargo build    # Build the backend
```

## Project Structure

```
caipi/
├── src/                          # Svelte frontend
│   ├── lib/
│   │   ├── assets/               # Static assets (caipi-logo.png)
│   │   ├── components/
│   │   │   ├── ui/               # Base components (Button, Card, Dialog, Titlebar, etc.)
│   │   │   ├── icons/            # SVG icon components (FolderIcon, SendIcon, ShieldIcon, etc.)
│   │   │   ├── sidebar/          # Sidebars (FileExplorer, ContextPanel, TaskList, SkillsList)
│   │   │   ├── onboarding/       # Welcome screen with CLI detection
│   │   │   ├── folder/           # Folder picker with drag-drop
│   │   │   ├── chat/             # Chat interface (ChatContainer, ChatMessage, MessageInput, ActivityCard)
│   │   │   └── permission/       # Permission modal
│   │   ├── stores/               # Svelte stores (app.ts, chat.ts, files.ts)
│   │   └── utils/                # Utilities (cn function)
│   ├── routes/                   # SvelteKit routes (+page.svelte, +layout.svelte)
│   └── app.css                   # Global styles + CSS variables
├── src-tauri/                    # Rust backend
│   ├── src/
│   │   ├── lib.rs                # Tauri app setup, command registration
│   │   ├── main.rs               # Entry point
│   │   ├── commands/             # Tauri commands
│   │   │   ├── setup.rs          # CLI detection (check_cli_installed, check_cli_authenticated)
│   │   │   ├── folder.rs         # Folder operations (get_recent_folders, save_recent_folder)
│   │   │   ├── files.rs          # File operations (list_directory)
│   │   │   └── chat.rs           # Chat/session management (create_session, send_message, set_permission_mode, set_model)
│   │   ├── claude/               # SDK integration
│   │   │   ├── agent.rs          # AgentSession wrapper, streaming events
│   │   │   └── permissions.rs    # Permission translation helpers
│   │   └── storage/              # Local data persistence
│   └── capabilities/default.json # Tauri permission capabilities
└── package.json
```

## Design System

### Brand Colors (from logo)
```
Dark blue-gray:  #122e38   (mascot outline, potential dark accents)
Forest green:    #439c3a   (deeper green tones)
Acid green:      #a9d80d   (bright lime highlights)
```

### UI Colors
```
Background:      #0d0d0d
Sidebar:         rgba(0, 0, 0, 0.2)
Card:            rgba(255, 255, 255, 0.02)
Border:          rgba(255, 255, 255, 0.06)
Hover:           rgba(255, 255, 255, 0.04)
Selected:        rgba(59, 130, 246, 0.15)

Text primary:    #e5e5e5
Text secondary:  #a3a3a3
Text muted:      #737373
Text dim:        #525252
Text darkest:    #404040

Accent blue:     #3b82f6
Folder purple:   #a78bfa
File gray:       #8b8b8b
```

### Typography
```
Base font:       -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", Roboto, sans-serif
Base size:       14px

Title large:     18px, weight 600  (main headings)
Title medium:    14px (text-sm), weight 600  (section headers)
Body:            14px  (message content, inputs, project names)
Small:           13px  (file tree items, task items)
Tiny:            12px  (labels, hints, timestamps, paths)
```

## Architecture

### Data Flow
1. User sends message via `MessageInput.svelte`
2. Frontend invokes `send_message` Tauri command
3. Rust backend sends to Claude SDK via `AgentSession`
4. SDK streams events back
5. Rust emits Tauri events (`claude:text`, `claude:tool_start`, etc.)
6. Frontend listens via `@tauri-apps/api/event` and updates UI

### Tauri Events
- `claude:text` - Streaming text chunks
- `claude:tool_start` - Tool execution started
- `claude:tool_end` - Tool execution completed
- `claude:permission_request` - Permission needed
- `claude:complete` - Turn complete
- `claude:error` - Error occurred

### State Management
- `app.ts` store: Current screen, selected folder, sidebar toggles, permission mode, model selection
- `chat.ts` store: Messages, tool activities, streaming state, tasks, skills, token count
- `files.ts` store: File tree state, expanded paths, selected file

### Permission Modes
Controlled via footer button in chat input. Affects how tool permissions are handled:
- **Default** (blue): Prompts for Write/Edit/Bash/NotebookEdit
- **Edit** (purple): Auto-allows file edits, prompts only for Bash
- **Plan** (green): Read-only, denies all write operations
- **Danger** (red): Bypasses all permission checks

### Model Selection
Controlled via footer button in chat input. Available models:
- **Opus 4.5** (large dot): `claude-opus-4-5-20251101`
- **Sonnet 4.5** (medium dot): `claude-sonnet-4-5-20250514`
- **Haiku 4.5** (small dot): `claude-haiku-3-5-20241022`

## Svelte 5 Runes

This project uses Svelte 5 runes syntax:
- `$state()` for reactive state
- `$derived()` for computed values
- `$effect()` for side effects
- `$props()` for component props

## UI Components

### Sidebars
- **Left sidebar (200px)**: File explorer with tree view, toggled via header button
- **Right sidebar (220px)**: Context panel with task list and active skills

### Chat Interface
- Role-based message labels (no avatars)
- Dividers between messages
- Input with focus outline feedback
- Footer with permission mode selector, model selector, and token/time stats

## Known Issues

1. **Unused Rust code**: `translate_permission`, `translate_bash_command` functions and `AgentError::Session` variant are defined but not yet used
2. **svelte:self deprecation**: FileTreeItem uses deprecated `<svelte:self>` for recursion (still functional)

## Tauri Permissions

Permissions are configured in `src-tauri/capabilities/default.json`. The app requires:
- `dialog:default`, `dialog:allow-open` - Native file dialogs
- `fs:default`, `fs:allow-read`, `fs:allow-write` - Filesystem access
- `opener:default` - Opening external URLs

## Testing Changes

After making changes:
1. Run `npm run tauri dev` to test
2. Check browser console for frontend errors
3. Check terminal for Rust backend logs/warnings
4. The app flow is: Onboarding → Folder Picker → Chat
