# Caipi - Claude Code Instructions

## Overview

Caipi is a macOS + Windows desktop app written in Tauri 2, providing a chat UI for Claude Code. Svelte 5 frontend, Rust backend.

## Repository

Single monorepo: `pietz/caipi` (private) at `/Users/pietz/Private/caipi`
- App source code in root
- Marketing website in `website/`

**URLs:**
- Website: https://caipi.ai
- Download: `https://github.com/pietz/caipi/releases/latest/download/caipi_aarch64.dmg`

## Commands

```bash
npm run tauri dev      # Run app in dev mode
npm run check          # Type check frontend
npm run test:all       # Run all tests

# Website (from website/ directory)
cd website && npm run dev    # Local dev server
cd website && npm run build  # Production build

# Test builds (manual, never publishes)
gh workflow run Build -f platform=windows  # Windows only
gh workflow run Build -f platform=macos    # macOS only
gh workflow run Build -f platform=both     # Both platforms
```

## GitHub Workflows

**Build** (manual only)
- Triggers: `gh workflow run Build -f platform=<macos|windows|both>`
- Builds unsigned artifacts, never publishes
- Use for: testing builds before release

**Release** (CI builds Windows only)
- Triggers: push to main with version change in package.json
- Builds Windows only (macOS is built locally to save CI costs)
- Does NOT publish — the local `scripts/release.sh` handles publishing

**Deploy Website**
- Triggers: push to main when `website/**` changes, or manual dispatch
- Builds Astro site from `website/` and deploys to GitHub Pages (caipi.ai)

## Release Process

When asked to release a new version:

1. Update the version in these 3 files:
   - `package.json`
   - `src-tauri/tauri.conf.json`
   - `src-tauri/Cargo.toml`
2. Commit and push to main (triggers Windows CI build)
3. Run `./scripts/release.sh` — this builds macOS locally (signed + notarized), waits for Windows CI to finish, and publishes the release with all artifacts

**Required env vars** for the release script (set in shell or `.env` file):
`APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`, `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

**After release**, update the release notes:
```bash
gh release edit v0.X.X --repo pietz/caipi --notes "## What's New

- Change 1
- Change 2"
```

## Backend CLI Changelogs

- Claude Code: https://github.com/anthropics/claude-code/releases
- Codex CLI: https://github.com/openai/codex/releases

## Tech Stack

- **Frontend**: Svelte 5 (runes) + TypeScript + SvelteKit + Tailwind CSS
- **Backend**: Rust + Tauri 2.0 (CLI-backed backends: Claude, Codex)

## Architecture

- **Data flow**: User message → Tauri command → Rust backend → CLI subprocess → `chat:event` stream → frontend UI
- **Events**: `chat:event`
- **Stores**: `app` (screen, folder, settings), `chat` (messages, tools, streaming), `files` (tree state)
- **Permission modes**: Default (prompts), Edit (auto-allow edits), Danger (bypass all)
- **Models**: Opus 4.6, Sonnet 4.5, Haiku 4.5

### Backend module structure

Each backend (`claude/`, `codex/`) follows the same layout:
- `adapter.rs` -- Session struct, process lifecycle (spawn/cleanup/abort), BackendSession trait impl, messaging protocol
- `event_handler.rs` -- Event dispatching and handling (`impl Session` block in a separate file, all associated functions taking Arc clones of state)
- `cli_protocol.rs` -- Wire types (serde structs for CLI JSON protocol)
- `sessions.rs` -- Session log parsing and history loading
- `hooks.rs` / `tool_utils.rs` -- Permission hooks, tool name/target extraction

Shared utilities live in `backends/utils.rs` (`abort_task_slot`, `spawn_stderr_drain`, `wait_for_permission`).

## Svelte 5 Runes

Uses `$state()`, `$derived()`, `$effect()`, `$props()` syntax.

## Brand Colors

- Dark blue-gray: `#122e38`
- Forest green: `#439c3a`
- Acid green: `#a9d80d`

## After Changes

1. Run `npm run test:all`
2. Test manually with `npm run tauri dev`
3. App flow: Onboarding → Folder Picker → Chat
