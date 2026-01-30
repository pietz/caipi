# Caipi - Claude Code Instructions

## Overview

Caipi is a macOS desktop app written in Tauri 2, providing a chat UI for Claude Code. Svelte 5 frontend, Rust backend.

## Repositories

| Repo | Purpose | Location |
|------|---------|----------|
| `pietz/caipi` (private) | Source code | `/Users/pietz/Private/caipi` |
| `pietz/caipi.ai` (public) | Releases, website, Homebrew tap | `/Users/pietz/Private/caipi.ai` |

**URLs:**
- Website: https://caipi.ai
- Download: `https://github.com/pietz/caipi.ai/releases/latest/download/caipi_aarch64.dmg`
- Homebrew cask: `caipi.ai/Casks/caipi.rb`

## Commands

```bash
# Development
npm run tauri dev      # Run app in dev mode
npm run check          # Type check frontend
npm run test:all       # Run all tests

# Release (requires `source .env` first for signing credentials)
npm run release                   # Build only
npm run release && npm run release:publish  # Build + publish to GitHub + update Homebrew
```

**Version bump**: Update `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml` before release.

## Tech Stack

- **Frontend**: Svelte 5 (runes) + TypeScript + SvelteKit + Tailwind CSS
- **Backend**: Rust + Tauri 2.0 + `claude-agent-sdk-rs`

## Architecture

- **Data flow**: User message → Tauri command → Rust SDK → streaming events → frontend UI
- **Events**: `claude:text`, `claude:tool_start`, `claude:tool_end`, `claude:permission_request`, `claude:complete`, `claude:error`
- **Stores**: `app` (screen, folder, settings), `chat` (messages, tools, streaming), `files` (tree state)
- **Permission modes**: Default (prompts), Edit (auto-allow edits), Danger (bypass all)
- **Models**: Opus 4.5, Sonnet 4.5, Haiku 4.5

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

## Release Notes

When publishing a release to `pietz/caipi.ai`, include a brief high-level changelog in the GitHub release notes. Focus on user-facing changes:
- New features or UI improvements
- Bug fixes
- Performance changes

Keep it concise (3-5 bullet points). No need for file-level details.
