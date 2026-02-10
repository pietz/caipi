# Caipi - Claude Code Instructions

## Overview

Caipi is a macOS + Windows desktop app written in Tauri 2, providing a chat UI for Claude Code. Svelte 5 frontend, Rust backend.

## Repositories

| Repo | Purpose | Location |
|------|---------|----------|
| `pietz/caipi` (private) | Source code | `/Users/pietz/Private/caipi` |
| `pietz/caipi.ai` (public) | Releases, website | `/Users/pietz/Private/caipi.ai` |

**URLs:**
- Website: https://caipi.ai
- Download: `https://github.com/pietz/caipi.ai/releases/latest/download/caipi_aarch64.dmg`

## Commands

```bash
npm run tauri dev      # Run app in dev mode
npm run check          # Type check frontend
npm run test:all       # Run all tests

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

**Release** (automatic)
- Triggers: push to main with version change in package.json
- Builds macOS (signed + notarized) + Windows
- Publishes to `pietz/caipi.ai` repo

## Release Process

When asked to release a new version, update the version in these 3 files:
- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`

Commit and push. GitHub Actions will automatically detect the version change and build/publish the release.

**After release**, update the release notes:
```bash
gh release edit v0.1.XX --repo pietz/caipi.ai --notes "## What's New

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
- **Events**: `chat:event`, `license:invalid`
- **Stores**: `app` (screen, folder, settings), `chat` (messages, tools, streaming), `files` (tree state)
- **Permission modes**: Default (prompts), Edit (auto-allow edits), Danger (bypass all)
- **Models**: Opus 4.6, Sonnet 4.5, Haiku 4.5

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
