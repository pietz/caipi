<p align="center">
  <img src="assets/caipi-logo-source.png" alt="Caipi" width="128" height="128">
</p>

<h1 align="center">Caipi</h1>

<p align="center">
  A fast, lightweight desktop app for AI coding CLIs.
  <br>
  <a href="https://caipi.ai">Website</a> &middot; <a href="https://github.com/pietz/caipi/releases/latest">Download</a>
</p>

<p align="center">
  <a href="https://github.com/pietz/caipi/releases/latest"><img src="https://img.shields.io/github/v/release/pietz/caipi?label=version" alt="Latest Release"></a>
  <a href="https://github.com/pietz/caipi/releases/latest"><img src="https://img.shields.io/github/downloads/pietz/caipi/total" alt="Downloads"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-BSL--1.1-blue" alt="License"></a>
</p>

---

<!-- TODO: Add product screenshot here -->

Chat apps just talk. Caipi gives [Claude Code](https://docs.anthropic.com/en/docs/claude-code) and [Codex CLI](https://github.com/openai/codex) a proper desktop interface -- so AI can read your files, run commands, and make changes, all with a visual layer that shows you what's happening.

No API keys needed. Caipi wraps the CLIs you already have installed, using your existing subscription.

## Features

- **File Explorer** -- Browse your project tree in a sidebar with real-time file watching. Double-click to open files in your default editor.
- **Session History** -- Pick up where you left off. Sessions are loaded from the CLI's own logs, grouped by project folder.
- **Permission Modes** -- Control what the AI can do. *Default* prompts for dangerous operations, *Edit* auto-allows file changes, *Allow All* bypasses everything.
- **Model Switching** -- Cycle between models (Opus, Sonnet, Haiku for Claude; GPT-5.x for Codex) without leaving the chat.
- **Extended Thinking** -- Toggle thinking depth (Low / Med / High) for models that support it.
- **Context Tracking** -- See how much of the context window is used at a glance.
- **Task & Skill Sidebar** -- Track agent todos and active skills in a collapsible panel.
- **Streaming** -- Real-time text and tool call display as the AI works.
- **Tool Visibility** -- Inline collapsible tool call stacks showing what the AI is reading, writing, and running.
- **Auto-Updates** -- Built-in update mechanism keeps you on the latest version.
- **Light & Dark Mode** -- Follows your system preference or set it manually.

## Installation

### Prerequisites

You need at least one of the following CLIs installed and authenticated:

- **[Claude Code](https://docs.anthropic.com/en/docs/claude-code)** -- Requires a Claude Pro or Max subscription.
- **[Codex CLI](https://github.com/openai/codex)** (optional) -- Requires an OpenAI API key or subscription.

Caipi detects installed backends automatically on startup.

### Download

| Platform | Download | Requirements |
|----------|----------|--------------|
| **macOS** | [Apple Silicon (.dmg)](https://github.com/pietz/caipi/releases/latest/download/caipi_aarch64.dmg) | macOS 12+ |
| **Windows** | [x64 (.exe)](https://github.com/pietz/caipi/releases/latest/download/caipi_x64.exe) | Windows 10+ |

Or grab the latest release from the [releases page](https://github.com/pietz/caipi/releases/latest).

## Getting Started

1. **Install** a supported CLI (Claude Code or Codex) and sign in.
2. **Download and open** Caipi.
3. The **setup wizard** will detect your installed backends. Pick your default.
4. **Choose a project folder** to work in.
5. **Start chatting** -- the AI can read your files, run commands, and make changes right on your machine.

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Enter` | Send message / Allow pending permission |
| `Shift+Enter` | New line in message input |
| `Escape` | Deny pending permission |

## Supported Backends

### Claude Code

| Model | Tier | Thinking |
|-------|------|----------|
| Opus 4.6 | Large | Low / Med / High |
| Sonnet 4.5 | Medium | Low / Med / High |
| Haiku 4.5 | Small | -- |

### Codex CLI

| Model | Tier | Thinking |
|-------|------|----------|
| GPT-5.3 Codex | Large | Low / Med / High |
| GPT-5.2 | Medium | Low / Med / High |
| GPT-5.1 Codex Mini | Small | -- |

## Roadmap

**Up Next**
- Multi-window support
- Attachment support
- Slash command support

**Exploring**
- Plan mode
- Linux support
- GitHub Copilot CLI support
- Gemini CLI support

## Tech Stack

- **Frontend**: Svelte 5, TypeScript, Tailwind CSS
- **Backend**: Rust, Tauri 2.0
- **Communication**: CLI subprocess over JSON stdin/stdout, streamed via Tauri events

## License

[Business Source License 1.1](LICENSE) -- Free for personal and non-commercial use. Commercial use requires a paid license. Converts to Apache 2.0 after four years.

Contact: [caipi@plpp.de](mailto:caipi@plpp.de)
