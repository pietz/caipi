# Caipi - Claude Code Instructions

## Overview

Caipi is a Tauri 2.0 desktop application that provides a friendly chat UI for Claude Code's agent capabilities. It wraps the Claude Code CLI with a native desktop interface.

## Tech Stack

- **Frontend**: Svelte 5 (with runes) + TypeScript + SvelteKit (static adapter)
- **Styling**: Tailwind CSS v3 + shadcn-style components with CSS variables
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
│   │   ├── components/
│   │   │   ├── ui/               # Base components (Button, Card, Dialog, etc.)
│   │   │   ├── onboarding/       # Welcome screen with CLI detection
│   │   │   ├── folder/           # Folder picker with drag-drop
│   │   │   ├── chat/             # Chat interface (ChatContainer, ChatMessage, MessageInput, ActivityCard)
│   │   │   └── permission/       # Permission modal
│   │   ├── stores/               # Svelte stores (app.ts, chat.ts)
│   │   └── utils/                # Utilities (cn function)
│   ├── routes/                   # SvelteKit routes (+page.svelte, +layout.svelte)
│   └── app.css                   # Global styles + Tailwind
├── src-tauri/                    # Rust backend
│   ├── src/
│   │   ├── lib.rs                # Tauri app setup, command registration
│   │   ├── main.rs               # Entry point
│   │   ├── commands/             # Tauri commands
│   │   │   ├── setup.rs          # CLI detection (check_cli_installed, check_cli_authenticated)
│   │   │   ├── folder.rs         # Folder operations (get_recent_folders, save_recent_folder)
│   │   │   └── chat.rs           # Chat/session management (create_session, send_message)
│   │   ├── claude/               # SDK integration
│   │   │   ├── agent.rs          # AgentSession wrapper, streaming events
│   │   │   └── permissions.rs    # Permission translation helpers
│   │   └── storage/              # Local data persistence
│   └── capabilities/default.json # Tauri permission capabilities
└── package.json
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
- `app.ts` store: Current screen, selected folder, settings
- `chat.ts` store: Messages, tool activities, streaming state

## Svelte 5 Runes

This project uses Svelte 5 runes syntax:
- `$state()` for reactive state
- `$derived()` for computed values
- `$effect()` for side effects
- `$props()` for component props

## Styling

Uses Tailwind CSS v3 with CSS variables for theming (shadcn-style):
- Colors defined in `app.css` as CSS variables (e.g., `--background`, `--foreground`)
- Components use `bg-background`, `text-foreground`, etc.
- Dark theme is default

## Known Issues

1. **Send button alignment**: The send button in MessageInput.svelte is not perfectly vertically aligned with the textarea
2. **Horizontal scroll bounce**: Elements show slight horizontal movement when scrolling (can fix with `overscroll-behavior: none`)
3. **Unused Rust code**: `translate_permission`, `translate_bash_command` functions and `AgentError::Session` variant are defined but not yet used
4. **Bundle size**: Main chunk is ~1.1MB (could benefit from code splitting)

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
