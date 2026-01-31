# Multi-Backend Architecture PRD

**Product Requirements Document for Caipi Multi-CLI Support**

Version: 1.0
Date: January 2026
Status: Planning

---

## Executive Summary

Caipi currently wraps Claude Code CLI via an unofficial Rust SDK (`claude-agent-sdk-rs`). This PRD outlines the architecture and implementation plan for supporting multiple AI coding CLI backends, starting with OpenAI Codex CLI, with future extensibility for Gemini CLI and GitHub Copilot CLI.

### Goals

1. **Support Codex CLI** as the second backend
2. **Maintain architectural consistency** with the current Claude Code integration
3. **Design for extensibility** to easily add future backends (Gemini, Copilot)
4. **Keep frontend mostly backend-agnostic** while allowing backend-specific UI elements
5. **Preserve all current features** with graceful handling of feature differences

### Non-Goals

- Hot-swapping backends mid-conversation (future consideration)
- Building custom agent loops (we wrap CLIs, not APIs directly)
- Supporting non-CLI backends (direct API integration)

---

## Research Summary

### Peer Review Sources

This plan incorporates perspectives from three LLMs:
- **Claude**: Detailed Codex CLI technical documentation and JSONL format analysis
- **Codex**: Rust architecture patterns, trait design, and multi-backend patterns
- **Gemini**: Alternative perspective favoring direct API integration (considered but rejected for architectural consistency)

### Decision: CLI Wrapper Approach

We chose to wrap `codex exec --json` rather than use `async-openai` directly because:
1. **Architectural consistency** - matches our Claude Code integration pattern
2. **Get agent capabilities free** - Codex handles tool execution, context management
3. **Simpler implementation** - spawn process + parse JSONL vs building agent loop
4. **Pragmatic path** - can swap to API later if needed via abstraction layer

---

## Technical Analysis

### Claude Code CLI (Current)

**Integration**: Via `claude-agent-sdk-rs` crate (v0.6)

**Event Stream**: SDK provides structured Rust types
- `Message::Assistant` with content blocks (text, tool_use, thinking)
- `Message::Result` for completion
- `Message::System` for auth info

**Tools**: Read, Write, Edit, Bash, Glob, Grep, WebFetch, WebSearch, Task, Skill, NotebookEdit, TodoWrite

**Permission Model**: Per-tool approval
- Default: Prompt for Write, Edit, Bash, NotebookEdit, Skill
- AcceptEdits: Auto-allow file operations, prompt for Bash
- BypassPermissions: Auto-allow all

**Thinking**: Binary toggle (`extended_thinking: bool`)

**Models**: opus, sonnet, haiku (mapped to claude-opus-4-5, claude-sonnet-4, etc.)

### Codex CLI

**Version**: 0.92.0 (research preview)

**Integration**: `codex exec --json` outputs JSONL to stdout

**JSONL Event Schema**:
```json
{"type":"thread.started","thread_id":"..."}
{"type":"turn.started"}
{"type":"item.started","item":{"id":"...","type":"command_execution","command":"...","status":"in_progress"}}
{"type":"item.completed","item":{"id":"...","type":"command_execution","command":"...","aggregated_output":"...","exit_code":0,"status":"completed"}}
{"type":"item.completed","item":{"id":"...","type":"reasoning","text":"..."}}
{"type":"item.completed","item":{"id":"...","type":"agent_message","text":"..."}}
{"type":"turn.completed","usage":{"input_tokens":...,"output_tokens":...}}
```

**Item Types**:
| Type | Description |
|------|-------------|
| `agent_message` | Text response from the model |
| `reasoning` | Thinking/reasoning summary |
| `command_execution` | Shell command with output |
| `file_change` | File creation/modification |
| `error` | Error or warning message |

**Sandbox Modes** (`-s, --sandbox`):
- `read-only` - No file writes
- `workspace-write` - Write within workspace only
- `danger-full-access` - No restrictions

**Approval Policies** (`-a, --ask-for-approval`):
- `untrusted` - Only trusted commands without asking
- `on-failure` - Ask only if command fails
- `on-request` - Model decides when to ask
- `never` - Never ask

**Reasoning Levels** (`model_reasoning_effort` in config):
- `low`, `medium`, `high`

**Models**: `gpt-5.2-codex` (default), supports `--oss` for local models

**Session Management**:
- `thread_id` returned in `thread.started` event
- Resume via `codex exec resume --last` or `codex exec resume <id>`

---

## Architecture Design

### Directory Structure

```
src-tauri/src/
├── backends/
│   ├── mod.rs              # BackendKind enum, Backend trait, registry
│   ├── types.rs            # Unified events, tools, capabilities
│   ├── session.rs          # AgentSession (backend-agnostic orchestrator)
│   ├── claude/
│   │   ├── mod.rs
│   │   ├── adapter.rs      # ClaudeBackend implementation
│   │   ├── parser.rs       # SDK messages → UnifiedEvent
│   │   └── settings.rs     # Claude-specific settings
│   └── codex/
│       ├── mod.rs
│       ├── adapter.rs      # CodexBackend implementation
│       ├── parser.rs       # JSONL → UnifiedEvent
│       └── settings.rs     # Codex-specific settings
├── commands/
│   ├── chat.rs             # Tauri commands (mostly unchanged)
│   └── setup.rs            # CLI detection for all backends
└── lib.rs                  # App setup with backend registry
```

### Core Types

#### Backend Kind

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    Claude,
    Codex,
    // Future: Gemini, Copilot
}
```

#### Unified Event Enum

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UnifiedEvent {
    // Text output
    Text { content: String },

    // Tool lifecycle
    ToolStart {
        tool_use_id: String,
        tool_type: String,        // Normalized: "bash", "file_read", "file_write", "file_edit"
        target: Option<String>,   // File path or command
        input: serde_json::Value,
    },
    ToolStatusUpdate {
        tool_use_id: String,
        status: ToolStatus,       // "pending", "running", "awaiting_permission", "completed", "error", "denied"
        permission_request_id: Option<String>,
    },
    ToolEnd {
        tool_use_id: String,
        status: ToolStatus,
        output: Option<String>,
    },

    // Thinking/reasoning
    ThinkingStart { thinking_id: String, content: Option<String> },
    ThinkingEnd { thinking_id: String },

    // Session lifecycle
    SessionInit { backend: BackendKind, session_id: String },
    StateChanged { /* backend-specific state */ },
    TokenUsage { input_tokens: u64, output_tokens: u64 },
    Complete,
    Error { message: String, recoverable: bool },

    // Escape hatch for backend-specific events
    BackendSpecific {
        backend: BackendKind,
        event_type: String,
        payload: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Pending,
    Running,
    AwaitingPermission,
    Completed,
    Error,
    Denied,
}
```

#### Tool Type Mapping

| Claude Tool | Codex Item Type | Unified Type |
|-------------|-----------------|--------------|
| `Bash` | `command_execution` | `bash` |
| `Read` | - | `file_read` |
| `Write` | `file_change` (create) | `file_write` |
| `Edit` | `file_change` (modify) | `file_edit` |
| `Glob` | - | `file_search` |
| `Grep` | - | `content_search` |
| `WebFetch` | - | `web_fetch` |
| `WebSearch` | `web_search` | `web_search` |
| `Task` | - | `agent_spawn` |

#### Backend Capabilities

```rust
#[derive(Debug, Clone, Serialize)]
pub struct BackendCapabilities {
    pub kind: BackendKind,
    pub display_name: String,
    pub icon: String,                    // Icon identifier for frontend

    // Feature flags
    pub supports_thinking: bool,
    pub supports_vision: bool,
    pub supports_web_search: bool,
    pub supports_session_resume: bool,

    // Permission model
    pub permission_model: PermissionModel,

    // Available models
    pub models: Vec<ModelInfo>,

    // Settings schema for dynamic UI
    pub settings_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionModel {
    PerTool,      // Claude: approve each tool
    Sandbox,      // Codex: sandbox levels + approval policy
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub icon: Option<String>,
    pub supports_thinking: bool,
}
```

#### Backend Trait

```rust
#[async_trait]
pub trait Backend: Send + Sync {
    fn kind(&self) -> BackendKind;
    fn capabilities(&self) -> BackendCapabilities;

    async fn check_installed(&self) -> Result<InstallStatus, BackendError>;
    async fn check_authenticated(&self) -> Result<AuthStatus, BackendError>;

    async fn create_session(
        &self,
        config: SessionConfig,
        app_handle: AppHandle,
    ) -> Result<Box<dyn BackendSession>, BackendError>;
}

#[async_trait]
pub trait BackendSession: Send + Sync {
    fn session_id(&self) -> &str;
    fn backend_kind(&self) -> BackendKind;

    async fn send_message(&mut self, message: &str) -> Result<(), BackendError>;
    async fn respond_permission(&mut self, request_id: &str, allowed: bool) -> Result<(), BackendError>;
    async fn interrupt(&mut self) -> Result<(), BackendError>;
    async fn disconnect(&mut self) -> Result<(), BackendError>;

    // Event stream
    fn events(&self) -> broadcast::Receiver<UnifiedEvent>;
}
```

#### Settings Types

```rust
// Common settings all backends share
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommonSettings {
    pub model: String,
}

// Claude-specific
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeSettings {
    #[serde(flatten)]
    pub common: CommonSettings,
    pub extended_thinking: bool,
    pub permission_mode: ClaudePermissionMode,  // "default", "acceptEdits", "bypassPermissions"
}

// Codex-specific
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexSettings {
    #[serde(flatten)]
    pub common: CommonSettings,
    pub sandbox_mode: CodexSandboxMode,        // "read-only", "workspace-write", "danger-full-access"
    pub approval_policy: CodexApprovalPolicy,  // "untrusted", "on-failure", "on-request", "never"
    pub reasoning_effort: ReasoningEffort,     // "low", "medium", "high"
}

// Discriminated union for storage/IPC
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "backend", rename_all = "snake_case")]
pub enum AnySettings {
    Claude(ClaudeSettings),
    Codex(CodexSettings),
}
```

### Permission Model Mapping

| Caipi Mode | Claude Setting | Codex Setting |
|------------|----------------|---------------|
| **Default** | `permission_mode: "default"` | `sandbox: "read-only"`, `approval: "untrusted"` |
| **Edit** | `permission_mode: "acceptEdits"` | `sandbox: "workspace-write"`, `approval: "on-request"` |
| **Danger** | `permission_mode: "bypassPermissions"` | `sandbox: "danger-full-access"`, `approval: "never"` |

Note: This mapping provides similar UX across backends but isn't a perfect 1:1. The frontend should show backend-appropriate labels when needed.

---

## Feature Mapping Layer

One of the central challenges of multi-backend support is handling features that exist in one backend but not another, or exist in both but work differently. This section defines the mapping layer that bridges these differences.

### Design Principles

1. **Unified UI concepts**: The frontend thinks in terms of abstract concepts (e.g., "thinking mode", "safety level"), not backend-specific settings.
2. **Backend adapters translate**: Each backend maps unified concepts to its native settings.
3. **Graceful degradation**: If a feature doesn't exist, hide it or use a sensible default.
4. **Superset, not intersection**: Support the union of all backend features, not just the common ones.

### Unified Settings Model

The frontend works with a `UnifiedSettings` struct that represents the superset of all configurable options:

```rust
/// Settings as the frontend understands them (backend-agnostic)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedSettings {
    // Common to all backends
    pub model: String,

    // Thinking/reasoning (different implementations per backend)
    pub thinking_mode: ThinkingMode,

    // Safety/permission level (abstracted from backend specifics)
    pub safety_level: SafetyLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingMode {
    Off,
    Low,      // Codex: reasoning_effort = low
    Medium,   // Codex: reasoning_effort = medium
    High,     // Codex: reasoning_effort = high; Claude: extended_thinking = true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyLevel {
    Default,  // Prompt for dangerous operations
    Edit,     // Allow file edits, prompt for shell
    Danger,   // Allow everything without prompts
}
```

### Backend Mapper Trait

Each backend implements a mapper that translates between unified and native settings:

```rust
pub trait BackendMapper {
    /// Convert unified settings to backend-native settings
    fn to_native(&self, unified: &UnifiedSettings) -> AnySettings;

    /// Convert backend-native settings to unified (for reading state back)
    fn from_native(&self, native: &AnySettings) -> UnifiedSettings;

    /// Which unified features does this backend support?
    fn supported_features(&self) -> FeatureSupport;

    /// Get the UI hints for this backend's settings
    fn ui_hints(&self) -> SettingsUiHints;
}

#[derive(Debug, Clone, Serialize)]
pub struct FeatureSupport {
    pub thinking: ThinkingSupport,
    pub safety_levels: Vec<SafetyLevel>,  // Which levels are supported
    pub session_resume: bool,
    pub vision: bool,
    pub web_search: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingSupport {
    None,                    // Backend doesn't support thinking
    Binary,                  // On/Off only (Claude)
    Levels(Vec<String>),     // Multiple levels (Codex: low/medium/high)
}
```

### Concrete Mappings

#### Thinking Mode Mapping

| Unified | Claude | Codex | Gemini (future) |
|---------|--------|-------|-----------------|
| `Off` | `extended_thinking: false` | `reasoning_effort: "low"` | TBD |
| `Low` | `extended_thinking: false` | `reasoning_effort: "low"` | TBD |
| `Medium` | `extended_thinking: false` | `reasoning_effort: "medium"` | TBD |
| `High` | `extended_thinking: true` | `reasoning_effort: "high"` | TBD |

Note: Claude only has binary thinking, so `Off`/`Low`/`Medium` all map to `false`. The UI should reflect this by showing a toggle for Claude, but a 3-level selector for Codex.

#### Safety Level Mapping

| Unified | Claude | Codex |
|---------|--------|-------|
| `Default` | `permission_mode: "default"` | `sandbox: "read-only"`, `approval: "untrusted"` |
| `Edit` | `permission_mode: "acceptEdits"` | `sandbox: "workspace-write"`, `approval: "on-request"` |
| `Danger` | `permission_mode: "bypassPermissions"` | `sandbox: "danger-full-access"`, `approval: "never"` |

#### Tool Type Mapping

| Unified Tool | Claude | Codex | Notes |
|--------------|--------|-------|-------|
| `bash` | `Bash` | `command_execution` | Direct mapping |
| `file_read` | `Read` | - | Codex may not emit separate read events |
| `file_write` | `Write` | `file_change` (new file) | Detect via file existence |
| `file_edit` | `Edit` | `file_change` (existing) | Detect via file existence |
| `file_search` | `Glob` | - | Claude-specific |
| `content_search` | `Grep` | - | Claude-specific |
| `web_search` | `WebSearch` | `web_search` | Direct mapping |
| `web_fetch` | `WebFetch` | - | Claude-specific |
| `agent_spawn` | `Task` | - | Claude-specific |

### Handling Missing Features

When a backend doesn't support a feature:

| Scenario | Behavior |
|----------|----------|
| Feature not supported | Hide the UI element entirely |
| Feature partially supported | Show with backend-appropriate options |
| Feature exists but named differently | Map silently, use unified terminology in UI |
| Feature has more options in one backend | Show extended options only for that backend |

### Implementation: Claude Mapper

```rust
impl BackendMapper for ClaudeMapper {
    fn to_native(&self, unified: &UnifiedSettings) -> AnySettings {
        AnySettings::Claude(ClaudeSettings {
            common: CommonSettings {
                model: self.map_model(&unified.model),
            },
            extended_thinking: matches!(unified.thinking_mode, ThinkingMode::High),
            permission_mode: match unified.safety_level {
                SafetyLevel::Default => ClaudePermissionMode::Default,
                SafetyLevel::Edit => ClaudePermissionMode::AcceptEdits,
                SafetyLevel::Danger => ClaudePermissionMode::BypassPermissions,
            },
        })
    }

    fn from_native(&self, native: &AnySettings) -> UnifiedSettings {
        let AnySettings::Claude(claude) = native else {
            panic!("Wrong backend type");
        };
        UnifiedSettings {
            model: claude.common.model.clone(),
            thinking_mode: if claude.extended_thinking {
                ThinkingMode::High
            } else {
                ThinkingMode::Off
            },
            safety_level: match claude.permission_mode {
                ClaudePermissionMode::Default => SafetyLevel::Default,
                ClaudePermissionMode::AcceptEdits => SafetyLevel::Edit,
                ClaudePermissionMode::BypassPermissions => SafetyLevel::Danger,
            },
        }
    }

    fn supported_features(&self) -> FeatureSupport {
        FeatureSupport {
            thinking: ThinkingSupport::Binary,
            safety_levels: vec![SafetyLevel::Default, SafetyLevel::Edit, SafetyLevel::Danger],
            session_resume: true,
            vision: true,
            web_search: true,
        }
    }

    fn ui_hints(&self) -> SettingsUiHints {
        SettingsUiHints {
            thinking_label: "Extended Thinking".to_string(),
            thinking_description: "Enable deep reasoning (slower, more thorough)".to_string(),
            safety_labels: HashMap::from([
                (SafetyLevel::Default, "Default".to_string()),
                (SafetyLevel::Edit, "Accept Edits".to_string()),
                (SafetyLevel::Danger, "Bypass Permissions".to_string()),
            ]),
        }
    }
}
```

### Implementation: Codex Mapper

```rust
impl BackendMapper for CodexMapper {
    fn to_native(&self, unified: &UnifiedSettings) -> AnySettings {
        AnySettings::Codex(CodexSettings {
            common: CommonSettings {
                model: self.map_model(&unified.model),
            },
            reasoning_effort: match unified.thinking_mode {
                ThinkingMode::Off | ThinkingMode::Low => ReasoningEffort::Low,
                ThinkingMode::Medium => ReasoningEffort::Medium,
                ThinkingMode::High => ReasoningEffort::High,
            },
            sandbox_mode: match unified.safety_level {
                SafetyLevel::Default => CodexSandboxMode::ReadOnly,
                SafetyLevel::Edit => CodexSandboxMode::WorkspaceWrite,
                SafetyLevel::Danger => CodexSandboxMode::DangerFullAccess,
            },
            approval_policy: match unified.safety_level {
                SafetyLevel::Default => CodexApprovalPolicy::Untrusted,
                SafetyLevel::Edit => CodexApprovalPolicy::OnRequest,
                SafetyLevel::Danger => CodexApprovalPolicy::Never,
            },
        })
    }

    fn supported_features(&self) -> FeatureSupport {
        FeatureSupport {
            thinking: ThinkingSupport::Levels(vec![
                "Low".to_string(),
                "Medium".to_string(),
                "High".to_string(),
            ]),
            safety_levels: vec![SafetyLevel::Default, SafetyLevel::Edit, SafetyLevel::Danger],
            session_resume: true,
            vision: false,  // Check if Codex supports vision
            web_search: true,
        }
    }

    fn ui_hints(&self) -> SettingsUiHints {
        SettingsUiHints {
            thinking_label: "Reasoning Effort".to_string(),
            thinking_description: "How much effort the model puts into reasoning".to_string(),
            safety_labels: HashMap::from([
                (SafetyLevel::Default, "Read Only".to_string()),
                (SafetyLevel::Edit, "Workspace Write".to_string()),
                (SafetyLevel::Danger, "Full Access".to_string()),
            ]),
        }
    }
}
```

### Frontend Usage

The frontend queries capabilities and renders accordingly:

```svelte
<script>
    const features = backend.supported_features();
    const hints = backend.ui_hints();
</script>

<!-- Thinking control adapts to backend -->
{#if features.thinking === 'binary'}
    <Toggle
        label={hints.thinking_label}
        bind:checked={settings.thinking_mode === 'high'}
    />
{:else if features.thinking.levels}
    <Select
        label={hints.thinking_label}
        options={features.thinking.levels}
        bind:value={settings.thinking_mode}
    />
{/if}

<!-- Safety level uses backend-specific labels -->
<SegmentedControl
    options={features.safety_levels.map(level => ({
        value: level,
        label: hints.safety_labels[level]
    }))}
    bind:value={settings.safety_level}
/>
```

### Adding a New Backend

When adding a new backend (e.g., Gemini CLI), the mapping layer requirements are:

1. **Implement `BackendMapper`** with translations for all unified settings
2. **Define `GeminiSettings`** with native configuration options
3. **Update `FeatureSupport`** to declare what the backend supports
4. **Provide `SettingsUiHints`** for appropriate labels/descriptions
5. **Add tool type mappings** in the event translator

The frontend requires no changes unless the new backend introduces entirely new concepts not in the unified model.

---

## Implementation Phases

### Phase 1: Abstraction Layer

**Goal**: Create the unified types and trait definitions without breaking existing functionality.

**Tasks**:
1. Create `src-tauri/src/backends/` directory structure
2. Define `BackendKind`, `UnifiedEvent`, `BackendCapabilities` types
3. Define `Backend` and `BackendSession` traits
4. Create `ClaudeBackend` adapter that wraps existing `AgentSession` logic
5. Move Claude-specific code from `src/claude/` to `src/backends/claude/`
6. Verify existing functionality still works

**Files Changed**:
- New: `backends/mod.rs`, `backends/types.rs`, `backends/claude/*.rs`
- Modified: `commands/chat.rs` (use new abstractions)
- Modified: `lib.rs` (register backends)

**Testing**: Run `npm run test:all`, manual test with `npm run tauri dev`

### Phase 2: Codex Backend Implementation

**Goal**: Implement `CodexBackend` that spawns `codex exec --json` and parses JSONL.

**Tasks**:
1. Implement `CodexBackend` struct with `Backend` trait
2. Implement JSONL parser for Codex events
3. Create event translator: Codex items → `UnifiedEvent`
4. Implement `CodexSession` with process spawning via `tokio::process`
5. Handle stdin for follow-up messages (or session resume)
6. Add Codex CLI detection to `commands/setup.rs`

**Key Implementation Details**:

```rust
// Spawning Codex CLI
let mut child = Command::new("codex")
    .args(["exec", "--json", "--skip-git-repo-check", "-C", &folder_path])
    .args(["-s", &sandbox_mode])
    .args(["-a", &approval_policy])
    .args(["-m", &model])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;

// Parse JSONL stream
let reader = BufReader::new(stdout);
for line in reader.lines() {
    let event: CodexEvent = serde_json::from_str(&line?)?;
    let unified = translate_codex_event(event);
    event_tx.send(unified)?;
}
```

**Files Changed**:
- New: `backends/codex/*.rs`
- Modified: `commands/setup.rs` (add `check_codex_installed`)

**Testing**: Unit tests for JSONL parsing, integration test with real Codex CLI

### Phase 3: Backend Selection & Onboarding

**Goal**: Allow users to select backend during onboarding.

**Tasks**:
1. Update `SetupWizard` component to show backend selection
2. Detect installed backends and show availability
3. Store selected backend in app settings
4. Pass backend selection to session creation
5. Update session picker to show backend-specific icons

**UI Changes**:
- Onboarding: Backend selector with icons (Claude logo, OpenAI logo)
- Installed backends are clickable, uninstalled are muted
- Brief description of each backend

**Files Changed**:
- Modified: `src/lib/components/setup/SetupWizard.svelte`
- Modified: `src/lib/stores/app.svelte.ts` (add `backend` field)
- Modified: `src/lib/api/tauri.ts` (add backend detection commands)
- New: Backend icon assets

### Phase 4: Backend-Specific UI Adaptations

**Goal**: Handle features that differ between backends.

**Tasks**:
1. **Model selector**: Show backend-appropriate models
2. **Permission controls**:
   - Claude: Show current 3-mode selector
   - Codex: Show sandbox mode + approval policy (or unified 3-mode)
3. **Thinking toggle**:
   - Claude: On/Off toggle
   - Codex: Low/Medium/High selector
4. **Session history**: Read Codex session history from `~/.codex/`

**Approach**: Use `BackendCapabilities` to conditionally render UI elements.

```svelte
{#if capabilities.permission_model === 'per_tool'}
  <ClaudePermissionSelector />
{:else if capabilities.permission_model === 'sandbox'}
  <CodexSandboxSelector />
{/if}
```

**Files Changed**:
- Modified: `src/lib/components/chat/ChatHeader.svelte`
- Modified: `src/lib/components/settings/PermissionSelector.svelte`
- New: `src/lib/components/settings/CodexSettings.svelte`

### Phase 5: Session Resume & History

**Goal**: Support resuming Codex sessions like Claude sessions.

**Tasks**:
1. Parse Codex session storage (`~/.codex/` or internal format)
2. Implement `codex exec resume <id>` flow
3. Unify session history UI to show both backends
4. Store `thread_id` from `thread.started` event

**Investigation Needed**: Codex session storage format (may need to examine `~/.codex/` structure)

---

## Frontend Event Handling

The frontend already listens to `claude:event` channel. With the unified event types, minimal changes are needed:

```typescript
// src/lib/utils/events.ts
function handleBackendEvent(event: UnifiedEvent) {
    switch(event.type) {
        case 'text':
            chat.appendText(event.content);
            break;
        case 'tool_start':
            chat.addTool(event.tool_use_id, event.tool_type, event.target);
            break;
        case 'tool_status_update':
            chat.updateToolStatus(event.tool_use_id, event.status);
            break;
        case 'tool_end':
            chat.updateToolStatus(event.tool_use_id, event.status);
            break;
        case 'thinking_start':
            chat.startThinking(event.thinking_id, event.content);
            break;
        case 'thinking_end':
            chat.endThinking(event.thinking_id);
            break;
        case 'complete':
            chat.finalize();
            break;
        case 'error':
            chat.addError(event.message);
            break;
        case 'backend_specific':
            // Log or handle backend-specific events as needed
            console.log(`Backend-specific event from ${event.backend}:`, event.payload);
            break;
    }
}
```

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Codex JSONL format changes | Medium | High | Pin CLI version, add format version detection, comprehensive error handling |
| Permission model mismatch | Medium | Medium | Use unified 3-mode approach, document differences |
| Session resume complexity | Medium | Low | Start without resume, add in Phase 5 |
| Performance overhead | Low | Low | JSONL parsing is fast, process spawning is same as current |
| Codex CLI stability | Medium | Medium | CLI wrapper isolates us from internal changes |

---

## Testing Strategy

### Unit Tests
- JSONL parser for all Codex event types
- Event translator: Codex → Unified
- Settings validation for both backends

### Integration Tests
- Spawn Codex CLI, send message, verify events
- Permission flow end-to-end
- Session creation/destruction

### Manual Testing
- Full conversation flow with both backends
- Tool execution (file edits, bash commands)
- Permission prompting
- Model switching
- Error scenarios

---

## Open Questions

1. **Codex session storage location**: Need to investigate where Codex stores session history for the resume feature.

2. **Multi-turn conversation in Codex**: Does `codex exec` support stdin for follow-up messages, or must we use `codex exec resume`?

3. **Codex MCP support**: Codex has experimental MCP server support. Should we expose this?

4. **Icon design**: Do we need new icons for the model selector, or can we adapt existing ones?

5. **Settings persistence**: Should backend-specific settings be stored per-folder or globally?

---

## Success Criteria

- [ ] Users can select Claude or Codex backend during onboarding
- [ ] Chat functionality works identically with both backends
- [ ] Tool execution (bash, file operations) works with both backends
- [ ] Permission prompting works appropriately for each backend
- [ ] Model selection shows backend-appropriate options
- [ ] Thinking/reasoning display works for both backends
- [ ] Session history shows sessions from both backends
- [ ] No regression in existing Claude Code functionality

---

## Appendix A: Codex JSONL Event Reference

### Event Types

| Type | Description |
|------|-------------|
| `thread.started` | New conversation started, contains `thread_id` |
| `turn.started` | Model turn begins |
| `turn.completed` | Model turn ends, contains `usage` |
| `item.started` | Tool/action starts, `status: "in_progress"` |
| `item.completed` | Tool/action completes |

### Item Types

| Type | Fields | Maps To |
|------|--------|---------|
| `agent_message` | `text` | `UnifiedEvent::Text` |
| `reasoning` | `text` | `UnifiedEvent::ThinkingEnd` |
| `command_execution` | `command`, `aggregated_output`, `exit_code`, `status` | `ToolStart` → `ToolEnd` |
| `file_change` | `path`, `content`, `status` | `ToolStart` → `ToolEnd` |
| `error` | `message` | `UnifiedEvent::Error` |

---

## Appendix B: CLI Command Reference

### Claude Code

```bash
# Check installation
which claude
claude --version

# Non-interactive (via SDK)
# SDK handles process spawning and JSON streaming
```

### Codex CLI

```bash
# Check installation
which codex
codex --version

# Non-interactive with JSON output
codex exec --json --skip-git-repo-check \
    -C /path/to/folder \
    -m gpt-5.2-codex \
    -s workspace-write \
    -a on-request \
    "Your prompt here"

# Resume session
codex exec resume <thread_id>
codex exec resume --last

# Check auth
codex login --status  # (if available)
```

---

## Appendix C: Architectural Patterns (from LLM Research)

### Consensus Patterns

All three LLMs (Claude, Codex, Gemini) agreed on:

1. **Unified event enum** with tagged variants + escape hatch for backend-specific data
2. **Capabilities struct** to advertise features per backend
3. **Translator pattern** - each backend maps raw events to unified events
4. **Settings with `#[serde(flatten)]`** for common + backend-specific fields

### Code Pattern: Settings Inheritance

```rust
#[derive(Clone, Serialize, Deserialize)]
pub struct ClaudeSettings {
    #[serde(flatten)]
    pub common: CommonSettings,

    // Claude-specific
    pub extended_thinking: bool,
    pub permission_mode: ClaudePermissionMode,
}
```

### Code Pattern: Event Translation

```rust
impl CodexBackend {
    fn translate_event(&self, raw: CodexEvent) -> UnifiedEvent {
        match raw {
            CodexEvent::ItemCompleted { item } => match item.item_type {
                ItemType::AgentMessage => UnifiedEvent::Text {
                    content: item.text.unwrap_or_default()
                },
                ItemType::CommandExecution => UnifiedEvent::ToolEnd {
                    tool_use_id: item.id,
                    status: if item.exit_code == Some(0) {
                        ToolStatus::Completed
                    } else {
                        ToolStatus::Error
                    },
                    output: item.aggregated_output,
                },
                // ... more mappings
            },
            // ... more event types
        }
    }
}
```
