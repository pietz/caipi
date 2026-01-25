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
│   │   │   ├── ui/               # Base components (Button, Card, Dialog, Spinner, Input, Textarea, ContextIndicator, ModelCircle)
│   │   │   ├── icons/            # Only CaipiIcon.svelte - all other icons use lucide-svelte
│   │   │   ├── sidebar/          # Sidebars (FileExplorer, FileTreeItem, ContextPanel, TodoList, SkillsList)
│   │   │   ├── onboarding/       # Welcome screen and SetupWizard with CLI detection
│   │   │   ├── folder/           # Folder picker with drag-drop
│   │   │   └── chat/             # Chat interface (ChatContainer, ChatMessage, MessageInput, ActivityCard, Divider)
│   │   ├── stores/               # Svelte 5 rune stores (app.svelte.ts, chat.svelte.ts, files.svelte.ts, theme.ts)
│   │   └── utils/                # Utilities (cn function, events)
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
- **Default**: Prompts for Write/Edit/Bash/NotebookEdit
- **Edit** (acceptEdits): Auto-allows file edits, prompts only for Bash
- **Danger** (bypassPermissions): Bypasses all permission checks

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
- Floating send button inside textarea (icon-only, theme-aware colors)
- Footer with model selector, permission mode selector, and context indicator

## Known Issues

1. **Unused Rust code**: `translate_permission`, `translate_bash_command` functions and `AgentError::Session` variant are defined but not yet used

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
