# Building AI-Powered Tauri v2 Apps with Svelte 5 and Claude Agent SDK

## Table of Contents

1. [Introduction](#introduction)
2. [Claude Agent SDK for Rust Overview](#claude-agent-sdk-for-rust-overview)
3. [Project Setup with Claude Agent SDK](#project-setup-with-claude-agent-sdk)
4. [Architecture: Tauri + Svelte 5 + Claude Agent SDK](#architecture-tauri--svelte-5--claude-agent-sdk)
5. [Rust Backend Integration Patterns](#rust-backend-integration-patterns)
6. [Svelte 5 Frontend Patterns for AI Features](#svelte-5-frontend-patterns-for-ai-features)
7. [Streaming Responses to the UI](#streaming-responses-to-the-ui)
8. [Session and Conversation Management](#session-and-conversation-management)
9. [Tool Integration and MCP Servers](#tool-integration-and-mcp-servers)
10. [Security Considerations](#security-considerations)
11. [Error Handling Best Practices](#error-handling-best-practices)
12. [State Management for AI Conversations](#state-management-for-ai-conversations)
13. [Performance Optimization](#performance-optimization)
14. [Common Pitfalls](#common-pitfalls)
15. [Complete Example: AI Chat Application](#complete-example-ai-chat-application)
16. [Quick Reference](#quick-reference)

---

## Introduction

This guide extends the Tauri v2 + Svelte 5 development patterns to incorporate the **Claude Agent SDK for Rust** (an unofficial community port by [tyrchen](https://github.com/tyrchen/claude-agent-sdk-rs)). This combination enables building desktop applications with native AI capabilities powered by Claude.

### Why This Stack?

| Component | Role | Benefit |
|-----------|------|---------|
| **Tauri v2** | Desktop runtime | Native performance, small binaries, secure IPC |
| **Svelte 5** | Frontend framework | Reactive UI with runes, minimal runtime |
| **Claude Agent SDK (Rust)** | AI integration | Type-safe Claude access, streaming, tools |

### What You Can Build

- AI-powered coding assistants with file system access
- Intelligent document processors
- Chat applications with tool integrations
- Automated workflow assistants
- Creative writing tools with context awareness

---

## Claude Agent SDK for Rust Overview

The `claude-agent-sdk-rs` crate by [tyrchen](https://github.com/tyrchen/claude-agent-sdk-rs) provides a Rust-native interface to Claude Code CLI, enabling:

### Key Features

- **One-shot Queries**: Simple `query()` function for single interactions
- **Bidirectional Streaming**: Real-time conversation with `ClaudeClient`
- **Tool Integration**: Custom MCP servers with the `tool!` macro
- **Hooks System**: Intercept and control Claude's behavior
- **Session Management**: Maintain context across conversations
- **Cost Control**: Budget limits and fallback models

### SDK Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Tauri Application                     │
├─────────────────────────────────────────────────────────┤
│  ┌─────────────────────┐     ┌─────────────────────────┐ │
│  │   Svelte 5 Frontend │◄───►│   Rust Backend          │ │
│  │   - Chat UI         │ IPC │   - Tauri Commands      │ │
│  │   - Runes State     │     │   - Claude Agent SDK    │ │
│  │   - Event Handlers  │     │   - MCP Servers         │ │
│  └─────────────────────┘     └─────────────────────────┘ │
├─────────────────────────────────────────────────────────┤
│                  Claude Agent SDK Layer                  │
│  ┌─────────────────────────────────────────────────────┐ │
│  │  ClaudeClient / query() → Claude Code CLI → Claude  │ │
│  └─────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

---

## Project Setup with Claude Agent SDK

### Prerequisites

1. **Rust 1.90+** with Tauri CLI
2. **Node.js 18+** for frontend tooling
3. **Claude Code CLI**: Install via `curl -fsSL https://claude.ai/install.sh | bash`
4. **Anthropic API Key** or Claude Code OAuth authentication

### Cargo.toml Configuration

```toml
[package]
name = "my-ai-app"
version = "0.1.0"
edition = "2021"

[dependencies]
tauri = { version = "2", features = ["protocol-asset"] }
tauri-plugin-shell = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
futures = "0.3"
anyhow = "1"

# Claude Agent SDK
claude-agent-sdk-rs = "0.6"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[profile.release]
lto = true
opt-level = "s"
strip = true
```

### Directory Structure

```
my-ai-tauri-app/
├── src/                          # SvelteKit frontend
│   ├── lib/
│   │   ├── components/
│   │   │   ├── Chat.svelte       # Chat interface
│   │   │   ├── Message.svelte    # Message display
│   │   │   └── ToolResult.svelte # Tool execution display
│   │   ├── stores/
│   │   │   ├── chat.svelte.ts    # Conversation state
│   │   │   └── settings.svelte.ts # AI settings
│   │   └── tauri/
│   │       ├── claude.ts         # Claude command wrappers
│   │       └── events.ts         # Streaming event handlers
│   └── routes/
│       ├── +layout.ts            # SSR disabled
│       └── +page.svelte          # Main chat page
├── src-tauri/
│   ├── src/
│   │   ├── main.rs               # Tauri entry
│   │   ├── lib.rs                # Command exports
│   │   ├── claude/
│   │   │   ├── mod.rs            # Claude integration
│   │   │   ├── client.rs         # ClaudeClient wrapper
│   │   │   ├── tools.rs          # Custom MCP tools
│   │   │   └── types.rs          # Shared types
│   │   └── commands/
│   │       ├── mod.rs
│   │       └── chat.rs           # Chat commands
│   ├── capabilities/
│   │   └── default.json          # Permissions
│   └── Cargo.toml
└── package.json
```

---

## Architecture: Tauri + Svelte 5 + Claude Agent SDK

### Communication Flow

```
┌──────────────────────────────────────────────────────────────────┐
│                         Svelte 5 Frontend                         │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │  let messages = $state<Message[]>([])                       │  │
│  │  let isStreaming = $state(false)                            │  │
│  │                                                              │  │
│  │  async function sendMessage(content: string) {              │  │
│  │    await invoke('send_message', { content })                │  │
│  │  }                                                           │  │
│  │                                                              │  │
│  │  $effect(() => {                                            │  │
│  │    const unlisten = listen('claude-response', (e) => {      │  │
│  │      messages.push(e.payload)                               │  │
│  │    })                                                        │  │
│  │    return () => unlisten.then(fn => fn())                   │  │
│  │  })                                                          │  │
│  └────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼ invoke() / listen()
┌──────────────────────────────────────────────────────────────────┐
│                         Tauri Commands                            │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │  #[tauri::command]                                          │  │
│  │  async fn send_message(                                     │  │
│  │    content: String,                                         │  │
│  │    state: State<'_, AppState>,                             │  │
│  │    app: AppHandle                                           │  │
│  │  ) -> Result<(), String> {                                  │  │
│  │    let client = state.claude_client.lock().await;          │  │
│  │    client.query(&content).await?;                          │  │
│  │    // Stream responses via events...                        │  │
│  │  }                                                           │  │
│  └────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼ Claude Agent SDK
┌──────────────────────────────────────────────────────────────────┐
│                      Claude Code CLI                              │
│                            ▼                                      │
│                      Claude API                                   │
└──────────────────────────────────────────────────────────────────┘
```

### Key Integration Points

1. **Tauri State**: Hold `ClaudeClient` in managed state for session persistence
2. **Commands**: Expose Claude operations as Tauri commands
3. **Events**: Stream Claude responses to frontend via Tauri events
4. **Channels**: Use Tauri channels for ordered message delivery

---

## Rust Backend Integration Patterns

### Basic Client Setup

```rust
// src-tauri/src/claude/client.rs
use claude_agent_sdk_rs::{ClaudeClient, ClaudeAgentOptions, PermissionMode};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct ClaudeClientWrapper {
    client: Arc<Mutex<Option<ClaudeClient>>>,
    options: ClaudeAgentOptions,
}

impl ClaudeClientWrapper {
    pub fn new() -> Self {
        let options = ClaudeAgentOptions {
            model: Some("sonnet".to_string()),
            permission_mode: Some(PermissionMode::AcceptEdits),
            max_turns: Some(10),
            ..Default::default()
        };
        
        Self {
            client: Arc::new(Mutex::new(None)),
            options,
        }
    }
    
    pub async fn connect(&self) -> anyhow::Result<()> {
        let mut client = ClaudeClient::new(self.options.clone());
        client.connect().await?;
        *self.client.lock().await = Some(client);
        Ok(())
    }
    
    pub async fn disconnect(&self) -> anyhow::Result<()> {
        if let Some(mut client) = self.client.lock().await.take() {
            client.disconnect().await?;
        }
        Ok(())
    }
}
```

### Tauri Commands for Claude Operations

```rust
// src-tauri/src/commands/chat.rs
use tauri::{AppHandle, Emitter, State};
use claude_agent_sdk_rs::{Message, ContentBlock};
use crate::claude::ClaudeClientWrapper;

#[derive(Clone, serde::Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub tool_use: Option<ToolUseInfo>,
}

#[derive(Clone, serde::Serialize)]
pub struct ToolUseInfo {
    pub name: String,
    pub input: serde_json::Value,
}

#[tauri::command]
pub async fn send_message(
    content: String,
    state: State<'_, ClaudeClientWrapper>,
    app: AppHandle,
) -> Result<(), String> {
    let client_guard = state.client.lock().await;
    let client = client_guard.as_ref()
        .ok_or("Claude client not connected")?;
    
    // Send query
    client.query(&content).await.map_err(|e| e.to_string())?;
    
    // Stream responses
    loop {
        match client.receive_message().await {
            Ok(Some(Message::Assistant(msg))) => {
                for block in msg.message.content {
                    match block {
                        ContentBlock::Text(text) => {
                            app.emit("claude-text", ChatMessage {
                                role: "assistant".to_string(),
                                content: text.text,
                                tool_use: None,
                            }).ok();
                        }
                        ContentBlock::ToolUse(tool) => {
                            app.emit("claude-tool-use", ChatMessage {
                                role: "tool".to_string(),
                                content: String::new(),
                                tool_use: Some(ToolUseInfo {
                                    name: tool.name,
                                    input: tool.input,
                                }),
                            }).ok();
                        }
                        _ => {}
                    }
                }
            }
            Ok(Some(Message::Result(result))) => {
                app.emit("claude-complete", result.result).ok();
                break;
            }
            Ok(None) => break,
            Ok(_) => continue,
            Err(e) => {
                app.emit("claude-error", e.to_string()).ok();
                break;
            }
        }
    }
    
    Ok(())
}

#[tauri::command]
pub async fn connect_claude(
    state: State<'_, ClaudeClientWrapper>,
) -> Result<(), String> {
    state.connect().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn disconnect_claude(
    state: State<'_, ClaudeClientWrapper>,
) -> Result<(), String> {
    state.disconnect().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn interrupt_claude(
    state: State<'_, ClaudeClientWrapper>,
) -> Result<(), String> {
    let client_guard = state.client.lock().await;
    if let Some(client) = client_guard.as_ref() {
        client.interrupt().await.map_err(|e| e.to_string())?;
    }
    Ok(())
}
```

### Main Application Setup

```rust
// src-tauri/src/lib.rs
mod claude;
mod commands;

use claude::ClaudeClientWrapper;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(ClaudeClientWrapper::new())
        .invoke_handler(tauri::generate_handler![
            commands::chat::send_message,
            commands::chat::connect_claude,
            commands::chat::disconnect_claude,
            commands::chat::interrupt_claude,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

---

## Svelte 5 Frontend Patterns for AI Features

### TypeScript Types

```typescript
// src/lib/tauri/types.ts
export interface ChatMessage {
  role: 'user' | 'assistant' | 'tool';
  content: string;
  toolUse?: {
    name: string;
    input: Record<string, unknown>;
  };
}

export interface ClaudeSettings {
  model: 'sonnet' | 'opus' | 'haiku';
  maxTurns: number;
  systemPrompt?: string;
}
```

### Chat State Management with Runes

```typescript
// src/lib/stores/chat.svelte.ts
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { ChatMessage } from '$lib/tauri/types';

class ChatStore {
  messages = $state<ChatMessage[]>([]);
  isConnected = $state(false);
  isStreaming = $state(false);
  error = $state<string | null>(null);
  
  private unlisteners: UnlistenFn[] = [];
  
  async connect() {
    try {
      await invoke('connect_claude');
      this.isConnected = true;
      await this.setupListeners();
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
    }
  }
  
  private async setupListeners() {
    // Text responses
    this.unlisteners.push(
      await listen<ChatMessage>('claude-text', (event) => {
        this.messages.push(event.payload);
      })
    );
    
    // Tool use
    this.unlisteners.push(
      await listen<ChatMessage>('claude-tool-use', (event) => {
        this.messages.push(event.payload);
      })
    );
    
    // Completion
    this.unlisteners.push(
      await listen('claude-complete', () => {
        this.isStreaming = false;
      })
    );
    
    // Errors
    this.unlisteners.push(
      await listen<string>('claude-error', (event) => {
        this.error = event.payload;
        this.isStreaming = false;
      })
    );
  }
  
  async sendMessage(content: string) {
    if (!this.isConnected || this.isStreaming) return;
    
    // Add user message
    this.messages.push({ role: 'user', content });
    this.isStreaming = true;
    this.error = null;
    
    try {
      await invoke('send_message', { content });
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
      this.isStreaming = false;
    }
  }
  
  async interrupt() {
    if (!this.isStreaming) return;
    
    try {
      await invoke('interrupt_claude');
      this.isStreaming = false;
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
    }
  }
  
  async disconnect() {
    // Clean up listeners
    for (const unlisten of this.unlisteners) {
      unlisten();
    }
    this.unlisteners = [];
    
    try {
      await invoke('disconnect_claude');
      this.isConnected = false;
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
    }
  }
  
  clearMessages() {
    this.messages = [];
    this.error = null;
  }
}

export const chatStore = new ChatStore();
```

### Chat Component

```svelte
<!-- src/lib/components/Chat.svelte -->
<script lang="ts">
  import { chatStore } from '$lib/stores/chat.svelte';
  import { onMount, onDestroy } from 'svelte';
  import Message from './Message.svelte';
  
  let inputValue = $state('');
  let messagesContainer: HTMLDivElement;
  
  // Derived state
  const canSend = $derived(
    chatStore.isConnected && 
    !chatStore.isStreaming && 
    inputValue.trim().length > 0
  );
  
  // Auto-scroll on new messages
  $effect(() => {
    // Access messages to track changes
    const _ = chatStore.messages.length;
    
    if (messagesContainer) {
      messagesContainer.scrollTop = messagesContainer.scrollHeight;
    }
  });
  
  onMount(async () => {
    await chatStore.connect();
  });
  
  onDestroy(async () => {
    await chatStore.disconnect();
  });
  
  async function handleSubmit(e: Event) {
    e.preventDefault();
    if (!canSend) return;
    
    const message = inputValue.trim();
    inputValue = '';
    await chatStore.sendMessage(message);
  }
  
  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit(e);
    }
  }
</script>

<div class="chat-container">
  <!-- Connection status -->
  <div class="status-bar">
    {#if !chatStore.isConnected}
      <span class="status disconnected">Connecting...</span>
    {:else if chatStore.isStreaming}
      <span class="status streaming">Claude is responding...</span>
      <button onclick={() => chatStore.interrupt()}>Stop</button>
    {:else}
      <span class="status connected">Connected</span>
    {/if}
  </div>
  
  <!-- Error display -->
  {#if chatStore.error}
    <div class="error-banner">
      {chatStore.error}
      <button onclick={() => chatStore.error = null}>Dismiss</button>
    </div>
  {/if}
  
  <!-- Messages -->
  <div class="messages" bind:this={messagesContainer}>
    {#each chatStore.messages as message}
      <Message {message} />
    {/each}
  </div>
  
  <!-- Input -->
  <form class="input-area" onsubmit={handleSubmit}>
    <textarea
      bind:value={inputValue}
      onkeydown={handleKeydown}
      placeholder="Type a message..."
      disabled={!chatStore.isConnected || chatStore.isStreaming}
    ></textarea>
    <button type="submit" disabled={!canSend}>
      Send
    </button>
  </form>
</div>

<style>
  .chat-container {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  
  .status-bar {
    padding: 0.5rem 1rem;
    background: var(--surface-2);
    display: flex;
    align-items: center;
    gap: 1rem;
  }
  
  .status {
    font-size: 0.875rem;
  }
  
  .status.connected { color: var(--green); }
  .status.streaming { color: var(--blue); }
  .status.disconnected { color: var(--orange); }
  
  .error-banner {
    padding: 0.75rem 1rem;
    background: var(--red-surface);
    color: var(--red);
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  
  .messages {
    flex: 1;
    overflow-y: auto;
    padding: 1rem;
  }
  
  .input-area {
    display: flex;
    gap: 0.5rem;
    padding: 1rem;
    border-top: 1px solid var(--border);
  }
  
  textarea {
    flex: 1;
    resize: none;
    padding: 0.75rem;
    border-radius: 0.5rem;
    border: 1px solid var(--border);
    min-height: 60px;
  }
  
  button {
    padding: 0.75rem 1.5rem;
    border-radius: 0.5rem;
    background: var(--primary);
    color: white;
    border: none;
    cursor: pointer;
  }
  
  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
```

### Message Component

```svelte
<!-- src/lib/components/Message.svelte -->
<script lang="ts">
  import type { ChatMessage } from '$lib/tauri/types';
  
  interface Props {
    message: ChatMessage;
  }
  
  let { message }: Props = $props();
  
  const roleLabel = $derived({
    user: 'You',
    assistant: 'Claude',
    tool: 'Tool',
  }[message.role]);
</script>

<div class="message {message.role}">
  <div class="role">{roleLabel}</div>
  
  {#if message.toolUse}
    <div class="tool-use">
      <div class="tool-name">{message.toolUse.name}</div>
      <pre class="tool-input">{JSON.stringify(message.toolUse.input, null, 2)}</pre>
    </div>
  {:else}
    <div class="content">{message.content}</div>
  {/if}
</div>

<style>
  .message {
    margin-bottom: 1rem;
    padding: 1rem;
    border-radius: 0.5rem;
  }
  
  .message.user {
    background: var(--user-bg);
    margin-left: 2rem;
  }
  
  .message.assistant {
    background: var(--assistant-bg);
    margin-right: 2rem;
  }
  
  .message.tool {
    background: var(--tool-bg);
    border-left: 3px solid var(--tool-accent);
    font-family: monospace;
  }
  
  .role {
    font-weight: 600;
    font-size: 0.875rem;
    margin-bottom: 0.5rem;
    color: var(--text-muted);
  }
  
  .content {
    white-space: pre-wrap;
    word-break: break-word;
  }
  
  .tool-use {
    font-size: 0.875rem;
  }
  
  .tool-name {
    font-weight: 600;
    color: var(--tool-accent);
  }
  
  .tool-input {
    margin-top: 0.5rem;
    padding: 0.5rem;
    background: var(--code-bg);
    border-radius: 0.25rem;
    overflow-x: auto;
  }
</style>
```

---

## Streaming Responses to the UI

### Using Tauri Channels for Ordered Delivery

For guaranteed message ordering, use Tauri's Channel API:

```rust
// src-tauri/src/commands/chat.rs
use tauri::ipc::Channel;

#[derive(Clone, serde::Serialize)]
#[serde(tag = "type", content = "data")]
pub enum StreamEvent {
    Text(String),
    ToolUse { name: String, input: serde_json::Value },
    Complete,
    Error(String),
}

#[tauri::command]
pub async fn send_message_stream(
    content: String,
    channel: Channel<StreamEvent>,
    state: State<'_, ClaudeClientWrapper>,
) -> Result<(), String> {
    let client_guard = state.client.lock().await;
    let client = client_guard.as_ref()
        .ok_or("Claude client not connected")?;
    
    client.query(&content).await.map_err(|e| e.to_string())?;
    
    loop {
        match client.receive_message().await {
            Ok(Some(Message::Assistant(msg))) => {
                for block in msg.message.content {
                    match block {
                        ContentBlock::Text(text) => {
                            channel.send(StreamEvent::Text(text.text)).ok();
                        }
                        ContentBlock::ToolUse(tool) => {
                            channel.send(StreamEvent::ToolUse {
                                name: tool.name,
                                input: tool.input,
                            }).ok();
                        }
                        _ => {}
                    }
                }
            }
            Ok(Some(Message::Result(_))) => {
                channel.send(StreamEvent::Complete).ok();
                break;
            }
            Ok(None) => break,
            Ok(_) => continue,
            Err(e) => {
                channel.send(StreamEvent::Error(e.to_string())).ok();
                break;
            }
        }
    }
    
    Ok(())
}
```

### Frontend Channel Handler

```typescript
// src/lib/tauri/claude.ts
import { invoke, Channel } from '@tauri-apps/api/core';

interface StreamEvent {
  type: 'Text' | 'ToolUse' | 'Complete' | 'Error';
  data?: string | { name: string; input: Record<string, unknown> };
}

export async function sendMessageWithStream(
  content: string,
  onText: (text: string) => void,
  onToolUse: (name: string, input: Record<string, unknown>) => void,
  onComplete: () => void,
  onError: (error: string) => void,
): Promise<void> {
  const channel = new Channel<StreamEvent>();
  
  channel.onmessage = (event) => {
    switch (event.type) {
      case 'Text':
        onText(event.data as string);
        break;
      case 'ToolUse':
        const tool = event.data as { name: string; input: Record<string, unknown> };
        onToolUse(tool.name, tool.input);
        break;
      case 'Complete':
        onComplete();
        break;
      case 'Error':
        onError(event.data as string);
        break;
    }
  };
  
  await invoke('send_message_stream', { content, channel });
}
```

---

## Session and Conversation Management

### Multiple Session Support

```rust
// src-tauri/src/claude/sessions.rs
use claude_agent_sdk_rs::ClaudeClient;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, ClaudeClient>>>,
    default_options: ClaudeAgentOptions,
}

impl SessionManager {
    pub fn new(options: ClaudeAgentOptions) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            default_options: options,
        }
    }
    
    pub async fn get_or_create(&self, session_id: &str) -> anyhow::Result<()> {
        let mut sessions = self.sessions.write().await;
        
        if !sessions.contains_key(session_id) {
            let mut client = ClaudeClient::new(self.default_options.clone());
            client.connect().await?;
            sessions.insert(session_id.to_string(), client);
        }
        
        Ok(())
    }
    
    pub async fn query(&self, session_id: &str, prompt: &str) -> anyhow::Result<()> {
        let sessions = self.sessions.read().await;
        let client = sessions.get(session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found"))?;
        
        client.query(prompt).await?;
        Ok(())
    }
    
    pub async fn clear_session(&self, session_id: &str) -> anyhow::Result<()> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(mut client) = sessions.remove(session_id) {
            client.disconnect().await?;
        }
        
        Ok(())
    }
}
```

### Frontend Session State

```typescript
// src/lib/stores/sessions.svelte.ts
import { invoke } from '@tauri-apps/api/core';

interface Session {
  id: string;
  name: string;
  createdAt: Date;
}

class SessionStore {
  sessions = $state<Session[]>([]);
  activeSessionId = $state<string | null>(null);
  
  async createSession(name: string): Promise<string> {
    const id = crypto.randomUUID();
    
    await invoke('create_session', { sessionId: id });
    
    this.sessions.push({
      id,
      name,
      createdAt: new Date(),
    });
    
    this.activeSessionId = id;
    return id;
  }
  
  async switchSession(sessionId: string) {
    this.activeSessionId = sessionId;
  }
  
  async deleteSession(sessionId: string) {
    await invoke('clear_session', { sessionId });
    
    this.sessions = this.sessions.filter(s => s.id !== sessionId);
    
    if (this.activeSessionId === sessionId) {
      this.activeSessionId = this.sessions[0]?.id ?? null;
    }
  }
}

export const sessionStore = new SessionStore();
```

---

## Tool Integration and MCP Servers

### Creating Custom Tools in Rust

```rust
// src-tauri/src/claude/tools.rs
use claude_agent_sdk_rs::{
    tool, create_sdk_mcp_server, ToolResult, McpToolResultContent,
    McpServers, McpServerConfig, SdkMcpServer,
};
use serde_json::json;
use std::path::PathBuf;

// File reading tool
async fn read_file_handler(args: serde_json::Value) -> anyhow::Result<ToolResult> {
    let path = args["path"].as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing path argument"))?;
    
    let content = tokio::fs::read_to_string(path).await?;
    
    Ok(ToolResult {
        content: vec![McpToolResultContent::Text { text: content }],
        is_error: false,
    })
}

// File writing tool
async fn write_file_handler(args: serde_json::Value) -> anyhow::Result<ToolResult> {
    let path = args["path"].as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing path argument"))?;
    let content = args["content"].as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing content argument"))?;
    
    tokio::fs::write(path, content).await?;
    
    Ok(ToolResult {
        content: vec![McpToolResultContent::Text { 
            text: format!("Successfully wrote to {}", path)
        }],
        is_error: false,
    })
}

// Search tool
async fn search_handler(args: serde_json::Value) -> anyhow::Result<ToolResult> {
    let query = args["query"].as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing query argument"))?;
    let directory = args["directory"].as_str().unwrap_or(".");
    
    // Implement search logic...
    let results = format!("Search results for '{}' in {}", query, directory);
    
    Ok(ToolResult {
        content: vec![McpToolResultContent::Text { text: results }],
        is_error: false,
    })
}

pub fn create_file_tools() -> SdkMcpServer {
    let read_tool = tool!(
        "read_file",
        "Read the contents of a file",
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read"
                }
            },
            "required": ["path"]
        }),
        read_file_handler
    );
    
    let write_tool = tool!(
        "write_file",
        "Write content to a file",
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to write to"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write"
                }
            },
            "required": ["path", "content"]
        }),
        write_file_handler
    );
    
    let search_tool = tool!(
        "search_files",
        "Search for files matching a pattern",
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "directory": {
                    "type": "string",
                    "description": "Directory to search in"
                }
            },
            "required": ["query"]
        }),
        search_handler
    );
    
    create_sdk_mcp_server(
        "file-tools",
        "1.0.0",
        vec![read_tool, write_tool, search_tool]
    )
}
```

### Configuring Tools with ClaudeClient

```rust
// src-tauri/src/claude/client.rs
use std::collections::HashMap;

pub fn create_claude_options_with_tools() -> ClaudeAgentOptions {
    let file_server = create_file_tools();
    
    let mut mcp_servers = HashMap::new();
    mcp_servers.insert("file-tools".to_string(), McpServerConfig::Sdk(file_server));
    
    ClaudeAgentOptions {
        model: Some("sonnet".to_string()),
        mcp_servers: McpServers::Dict(mcp_servers),
        // Grant permission for MCP tools
        allowed_tools: vec![
            "mcp__file-tools__read_file".to_string(),
            "mcp__file-tools__write_file".to_string(),
            "mcp__file-tools__search_files".to_string(),
        ],
        permission_mode: Some(PermissionMode::AcceptEdits),
        ..Default::default()
    }
}
```

---

## Security Considerations

### Tauri Capability Configuration

```json
// src-tauri/capabilities/default.json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default capability for AI chat",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "shell:allow-spawn",
    {
      "identifier": "shell:allow-execute",
      "allow": [
        { "name": "claude", "cmd": "claude", "args": true }
      ]
    }
  ]
}
```

### API Key Management

```rust
// src-tauri/src/claude/auth.rs
use std::env;

pub fn get_api_key() -> Option<String> {
    // Priority: environment variable > config file
    env::var("ANTHROPIC_API_KEY").ok()
        .or_else(|| load_from_config())
}

fn load_from_config() -> Option<String> {
    let config_path = dirs::config_dir()?
        .join("my-app")
        .join("config.json");
    
    let content = std::fs::read_to_string(config_path).ok()?;
    let config: serde_json::Value = serde_json::from_str(&content).ok()?;
    
    config["api_key"].as_str().map(String::from)
}
```

### Input Validation

```rust
// Validate user input before sending to Claude
fn validate_prompt(prompt: &str) -> Result<(), String> {
    if prompt.is_empty() {
        return Err("Prompt cannot be empty".to_string());
    }
    
    if prompt.len() > 100_000 {
        return Err("Prompt too long".to_string());
    }
    
    // Add more validation as needed
    Ok(())
}
```

---

## Error Handling Best Practices

### Rust Error Types

```rust
// src-tauri/src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Claude client not connected")]
    NotConnected,
    
    #[error("Claude CLI not found. Install with: curl -fsSL https://claude.ai/install.sh | bash")]
    CliNotFound,
    
    #[error("API key not configured")]
    NoApiKey,
    
    #[error("Session '{0}' not found")]
    SessionNotFound(String),
    
    #[error("Claude error: {0}")]
    ClaudeError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

// Make serializable for Tauri
impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
```

### Frontend Error Handling

```typescript
// src/lib/stores/chat.svelte.ts
type ErrorType = 
  | 'connection'
  | 'api_key'
  | 'cli_not_found'
  | 'rate_limit'
  | 'unknown';

interface AppError {
  type: ErrorType;
  message: string;
  recoverable: boolean;
}

function parseError(error: unknown): AppError {
  const message = error instanceof Error ? error.message : String(error);
  
  if (message.includes('not connected')) {
    return { type: 'connection', message, recoverable: true };
  }
  
  if (message.includes('CLI not found')) {
    return { type: 'cli_not_found', message, recoverable: false };
  }
  
  if (message.includes('API key')) {
    return { type: 'api_key', message, recoverable: false };
  }
  
  if (message.includes('rate limit')) {
    return { type: 'rate_limit', message, recoverable: true };
  }
  
  return { type: 'unknown', message, recoverable: false };
}
```

---

## State Management for AI Conversations

### Combining Svelte 5 Runes with Tauri

```typescript
// src/lib/stores/ai-state.svelte.ts
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// Settings that persist across sessions
class AISettings {
  model = $state<'sonnet' | 'opus' | 'haiku'>('sonnet');
  maxTurns = $state(10);
  systemPrompt = $state('');
  
  // Derived
  readonly modelDisplay = $derived({
    sonnet: 'Claude Sonnet (Balanced)',
    opus: 'Claude Opus (Most Capable)',
    haiku: 'Claude Haiku (Fastest)',
  }[this.model]);
  
  async save() {
    await invoke('save_settings', {
      settings: {
        model: this.model,
        maxTurns: this.maxTurns,
        systemPrompt: this.systemPrompt,
      }
    });
  }
  
  async load() {
    const settings = await invoke<{
      model: 'sonnet' | 'opus' | 'haiku';
      maxTurns: number;
      systemPrompt: string;
    }>('load_settings');
    
    this.model = settings.model;
    this.maxTurns = settings.maxTurns;
    this.systemPrompt = settings.systemPrompt;
  }
}

export const aiSettings = new AISettings();

// Conversation state (ephemeral)
class ConversationState {
  messages = $state<Message[]>([]);
  isStreaming = $state(false);
  currentToolUse = $state<ToolUse | null>(null);
  
  // Computed stats
  readonly messageCount = $derived(this.messages.length);
  readonly tokenEstimate = $derived(
    this.messages.reduce((sum, m) => sum + m.content.length / 4, 0)
  );
  
  addMessage(message: Message) {
    this.messages.push(message);
  }
  
  clear() {
    this.messages = [];
    this.currentToolUse = null;
  }
}

export const conversation = new ConversationState();
```

---

## Performance Optimization

### Rust Optimizations

```toml
# Cargo.toml
[profile.release]
lto = true
opt-level = "s"
strip = true
codegen-units = 1
panic = "abort"

[profile.dev]
opt-level = 1  # Faster dev builds with some optimization
```

### Efficient Message Streaming

```rust
// Use query_stream for memory-efficient processing
use claude_agent_sdk_rs::query_stream;
use futures::StreamExt;

pub async fn stream_query(
    prompt: &str,
    app: &AppHandle,
) -> anyhow::Result<()> {
    let mut stream = query_stream(prompt, None).await?;
    
    // Process messages one at a time (O(1) memory)
    while let Some(result) = stream.next().await {
        let message = result?;
        // Emit to frontend immediately
        app.emit("claude-message", &message).ok();
    }
    
    Ok(())
}
```

### Frontend Virtual Scrolling for Long Conversations

```svelte
<!-- src/lib/components/VirtualMessageList.svelte -->
<script lang="ts">
  import { chatStore } from '$lib/stores/chat.svelte';
  
  let containerHeight = $state(0);
  let scrollTop = $state(0);
  
  const itemHeight = 80; // Estimated message height
  const overscan = 5;
  
  const visibleRange = $derived(() => {
    const start = Math.max(0, Math.floor(scrollTop / itemHeight) - overscan);
    const visibleCount = Math.ceil(containerHeight / itemHeight) + overscan * 2;
    const end = Math.min(chatStore.messages.length, start + visibleCount);
    return { start, end };
  });
  
  const visibleMessages = $derived(
    chatStore.messages.slice(visibleRange.start, visibleRange.end)
  );
  
  const totalHeight = $derived(chatStore.messages.length * itemHeight);
  const offsetY = $derived(visibleRange.start * itemHeight);
</script>

<div 
  class="virtual-list"
  bind:clientHeight={containerHeight}
  onscroll={(e) => scrollTop = e.currentTarget.scrollTop}
>
  <div style="height: {totalHeight}px; position: relative;">
    <div style="transform: translateY({offsetY}px);">
      {#each visibleMessages as message, i (visibleRange.start + i)}
        <Message {message} />
      {/each}
    </div>
  </div>
</div>
```

---

## Common Pitfalls

### 1. Not Cleaning Up Claude Client on Window Close

**Wrong:**
```rust
// Client stays connected when app closes
```

**Correct:**
```rust
// src-tauri/src/lib.rs
.on_window_event(|window, event| {
    if let tauri::WindowEvent::CloseRequested { .. } = event {
        let state = window.state::<ClaudeClientWrapper>();
        tauri::async_runtime::block_on(async {
            state.disconnect().await.ok();
        });
    }
})
```

### 2. Blocking the Main Thread with Claude Queries

**Wrong:**
```rust
#[tauri::command]
fn query_claude(prompt: String) -> String {
    // This blocks!
    futures::executor::block_on(query(&prompt, None))
}
```

**Correct:**
```rust
#[tauri::command]
async fn query_claude(prompt: String) -> Result<String, String> {
    // Async - doesn't block
    let messages = query(&prompt, None).await
        .map_err(|e| e.to_string())?;
    // ...
}
```

### 3. Missing Listener Cleanup in Svelte

**Wrong:**
```typescript
$effect(() => {
  listen('claude-response', handler); // Never cleaned up!
});
```

**Correct:**
```typescript
$effect(() => {
  const unlistenPromise = listen('claude-response', handler);
  
  return () => {
    unlistenPromise.then(unlisten => unlisten());
  };
});
```

### 4. Using $effect for Derived State

**Wrong:**
```typescript
let messageCount = $state(0);

$effect(() => {
  messageCount = messages.length; // Don't use $effect for this!
});
```

**Correct:**
```typescript
const messageCount = $derived(messages.length);
```

### 5. Not Handling CLI Not Found Error

**Wrong:**
```rust
let messages = query(&prompt, None).await?; // Unclear error
```

**Correct:**
```rust
match query(&prompt, None).await {
    Ok(messages) => { /* handle */ }
    Err(e) if e.to_string().contains("CLI not found") => {
        return Err("Claude Code CLI not installed. Run: curl -fsSL https://claude.ai/install.sh | bash".into());
    }
    Err(e) => return Err(e.to_string()),
}
```

### 6. Forgetting to Allow MCP Tools

**Wrong:**
```rust
// Tools won't work without explicit permission
let options = ClaudeAgentOptions {
    mcp_servers: McpServers::Dict(servers),
    // Missing allowed_tools!
    ..Default::default()
};
```

**Correct:**
```rust
let options = ClaudeAgentOptions {
    mcp_servers: McpServers::Dict(servers),
    allowed_tools: vec![
        "mcp__my-server__my-tool".to_string(),
    ],
    ..Default::default()
};
```

---

## Complete Example: AI Chat Application

### Project Files Overview

```
ai-chat-app/
├── src/
│   ├── app.css
│   ├── app.html
│   ├── lib/
│   │   ├── components/
│   │   │   ├── Chat.svelte
│   │   │   ├── Message.svelte
│   │   │   ├── Settings.svelte
│   │   │   └── Sidebar.svelte
│   │   ├── stores/
│   │   │   ├── chat.svelte.ts
│   │   │   └── settings.svelte.ts
│   │   └── tauri/
│   │       ├── claude.ts
│   │       └── types.ts
│   └── routes/
│       ├── +layout.svelte
│       ├── +layout.ts
│       └── +page.svelte
└── src-tauri/
    ├── Cargo.toml
    ├── capabilities/default.json
    └── src/
        ├── claude/
        │   ├── mod.rs
        │   ├── client.rs
        │   └── tools.rs
        ├── commands/
        │   ├── mod.rs
        │   └── chat.rs
        ├── lib.rs
        └── main.rs
```

### Key Configuration Files

```typescript
// src/routes/+layout.ts
export const ssr = false;
export const prerender = false;
```

```json
// src-tauri/capabilities/default.json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "core:event:default",
    "shell:allow-spawn"
  ]
}
```

---

## Quick Reference

### Claude Agent SDK Rust Cheat Sheet

```rust
// One-shot query
use claude_agent_sdk_rs::query;
let messages = query("Hello", None).await?;

// Streaming query
use claude_agent_sdk_rs::query_stream;
let mut stream = query_stream("Hello", None).await?;
while let Some(msg) = stream.next().await { /* ... */ }

// Bidirectional client
use claude_agent_sdk_rs::ClaudeClient;
let mut client = ClaudeClient::new(options);
client.connect().await?;
client.query("Hello").await?;
// ... receive messages ...
client.disconnect().await?;

// Options
let options = ClaudeAgentOptions {
    model: Some("sonnet".to_string()),
    max_turns: Some(10),
    permission_mode: Some(PermissionMode::AcceptEdits),
    tools: Some(Tools::List(vec!["Read".to_string()])),
    allowed_tools: vec!["mcp__server__tool".to_string()],
    ..Default::default()
};

// Custom tool
let tool = tool!("name", "description", json!({...}), handler_fn);
let server = create_sdk_mcp_server("server-name", "1.0.0", vec![tool]);
```

### Svelte 5 + Tauri Patterns

```typescript
// State
let value = $state(0);

// Derived
const doubled = $derived(value * 2);

// Effect with cleanup
$effect(() => {
  const unlisten = listen('event', handler);
  return () => unlisten.then(fn => fn());
});

// Tauri invoke
await invoke('command_name', { arg: value });

// Tauri channel
const channel = new Channel<Event>();
channel.onmessage = (e) => { /* ... */ };
await invoke('streaming_command', { channel });
```

### Common Tauri Commands Structure

```rust
#[tauri::command]
async fn my_command(
    arg: String,
    state: State<'_, MyState>,
    app: AppHandle,
) -> Result<Response, String> {
    // Implementation
}
```

---

## Resources

- [tyrchen/claude-agent-sdk-rs](https://github.com/tyrchen/claude-agent-sdk-rs) - Rust SDK
- [Anthropic Claude Agent SDK Docs](https://platform.claude.com/docs/en/agent-sdk/overview)
- [Tauri v2 Documentation](https://v2.tauri.app)
- [Svelte 5 Runes](https://svelte.dev/docs/svelte/$state)
- [Original Tauri + Svelte Guide](/mnt/user-data/outputs/tauri-v2-svelte-5-guide.md)

---

*This guide combines patterns from Tauri v2, Svelte 5, and the unofficial Claude Agent SDK for Rust to help you build AI-powered desktop applications.*