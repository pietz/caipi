//! Claude CLI JSON Protocol Types
//!
//! This module defines the serde types for the Claude CLI's JSON streaming protocol.
//! The protocol uses newline-delimited JSON (NDJSON) for bidirectional communication
//! between the client and the Claude CLI.
//!
//! ## Protocol Overview
//!
//! The CLI emits events in a streaming format:
//! - `System` messages for initialization and health checks
//! - `Assistant` messages containing Claude's responses (text, tool_use, thinking)
//! - `User` messages for tool results (when replay-user-messages is enabled)
//! - `Result` messages indicating completion (success or error)
//!
//! Control protocol messages handle permission requests and responses:
//! - The CLI sends `ControlRequest` messages when it needs permission for tools
//! - The client responds with `ControlResponse` messages containing the decision
//!
//! ## Usage
//!
//! These types are designed to be used with serde for JSON serialization/deserialization:
//!
//! ```rust,ignore
//! let event: CliEvent = serde_json::from_str(line)?;
//! match event {
//!     CliEvent::Assistant(msg) => { /* handle assistant message */ }
//!     CliEvent::Result(result) => { /* handle completion */ }
//!     // ...
//! }
//! ```

// These types are protocol definitions for future use
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ============================================================================
// Main Event Enum
// ============================================================================

/// Top-level event types emitted by the Claude CLI.
///
/// Each event has a `type` field used for tagged enum deserialization.
/// The CLI streams these events as newline-delimited JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CliEvent {
    /// System events for initialization and health checks.
    System(SystemEvent),

    /// Assistant messages containing Claude's responses.
    /// Includes text content, tool use requests, and thinking blocks.
    Assistant(AssistantEvent),

    /// User messages containing tool results.
    /// Only emitted when `--replay-user-messages` flag is used.
    User(UserEvent),

    /// Result event indicating the turn has completed.
    /// Contains success/error status and usage information.
    Result(ResultEvent),

    /// Control request from CLI requiring a response (e.g., hook callbacks).
    ControlRequest(IncomingControlRequest),

    /// Acknowledgment of our control response.
    ControlResponse(ControlResponseAck),

    /// Catch-all for unknown event types added in future CLI versions.
    /// Prevents deserialization failures when the CLI adds new event types.
    #[serde(other)]
    Unknown,
}

// ============================================================================
// System Events
// ============================================================================

/// System event for CLI initialization and health checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemEvent {
    /// Subtype of the system event (e.g., "init", "health_check")
    pub subtype: String,

    /// Session ID assigned by the CLI
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    /// Additional data specific to the event subtype
    #[serde(default, flatten)]
    pub data: Value,
}

/// System event subtypes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemSubtype {
    /// Initial connection event with session info
    Init,
    /// Periodic health check
    HealthCheck,
}

// ============================================================================
// Assistant Events
// ============================================================================

/// Assistant event containing Claude's response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantEvent {
    /// The message content from the assistant
    pub message: AssistantMessage,
}

/// A message from the Claude assistant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    /// Role is always "assistant" for these messages
    pub role: String,

    /// Content blocks containing text, tool uses, or thinking
    pub content: Vec<ContentBlock>,

    /// The model that generated this response (e.g., "claude-sonnet-4-5")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Reason the model stopped generating (e.g., "end_turn", "tool_use")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,

    /// Token usage for this API call
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageInfo>,
}

// ============================================================================
// Content Blocks
// ============================================================================

/// Content block types that can appear in assistant messages.
///
/// Claude's responses are composed of multiple content blocks,
/// which can include plain text, tool use requests, or thinking content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text content from Claude
    Text(TextBlock),

    /// A request to use a tool
    ToolUse(ToolUseBlock),

    /// Extended thinking content (requires thinking mode)
    Thinking(ThinkingBlock),

    /// Streaming delta for tool input JSON (partial updates)
    InputJsonDelta(InputJsonDeltaBlock),

    /// Tool result block (appears in user messages)
    ToolResult(ToolResultBlock),

    /// Catch-all for unknown content block types added in future CLI versions.
    /// Prevents a single unknown block from dropping the entire assistant message.
    #[serde(other)]
    Unknown,
}

/// Text content block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlock {
    /// The text content
    pub text: String,
}

/// Tool use request block.
///
/// Claude requests to use a tool by emitting this block.
/// The client must execute the tool and return a `ToolResult`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseBlock {
    /// Unique identifier for this tool use (used to match results)
    pub id: String,

    /// Name of the tool to invoke (e.g., "Read", "Write", "Bash")
    pub name: String,

    /// Input parameters for the tool as JSON
    pub input: Value,
}

/// Extended thinking block.
///
/// Contains Claude's reasoning process when extended thinking is enabled.
/// These blocks are emitted before the final response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingBlock {
    /// The thinking content (Claude's reasoning)
    pub thinking: String,

    /// Signature for thinking verification (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

/// Streaming delta for tool input JSON.
///
/// Used during streaming to provide incremental updates to tool input.
/// The client should accumulate these deltas to build the complete input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputJsonDeltaBlock {
    /// Partial JSON string to append
    pub partial_json: String,
}

/// Tool result block (appears in user messages).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultBlock {
    /// ID of the tool use this result corresponds to
    pub tool_use_id: String,

    /// Result content (can be string or structured)
    pub content: Value,

    /// Whether the tool execution resulted in an error
    #[serde(default)]
    pub is_error: bool,
}

// ============================================================================
// User Events
// ============================================================================

/// User event containing tool results.
///
/// Emitted when `--replay-user-messages` is enabled.
/// Contains the results of tool executions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserEvent {
    /// Additional data including the nested message with tool results
    #[serde(flatten)]
    pub extra: Value,
}

/// A user message (typically containing tool results).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    /// Role is "user" for these messages
    pub role: String,

    /// Content blocks (usually tool_result blocks)
    pub content: Vec<ContentBlock>,
}

// ============================================================================
// Result Events
// ============================================================================

/// Result event indicating turn completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultEvent {
    /// Subtype: "success" or "error"
    pub subtype: String,

    /// Cost in USD for this turn (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,

    /// Whether the turn was aborted
    #[serde(default)]
    pub is_aborted: bool,

    /// Total duration in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,

    /// Total API duration in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_api_ms: Option<u64>,

    /// Number of API turns
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_turns: Option<u32>,

    /// Session ID for resuming later
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    /// Cumulative token usage for the entire session
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_cost: Option<CostInfo>,
}

/// Result subtypes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultSubtype {
    /// Turn completed successfully
    Success,
    /// Turn ended with an error
    Error,
}

// ============================================================================
// Usage and Cost Information
// ============================================================================

/// Token usage information for an API call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageInfo {
    /// Number of input tokens
    #[serde(default)]
    pub input_tokens: u64,

    /// Number of output tokens
    #[serde(default)]
    pub output_tokens: u64,

    /// Number of input tokens read from cache
    #[serde(default)]
    pub cache_read_input_tokens: u64,

    /// Number of input tokens written to cache
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
}

/// Cost information for a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostInfo {
    /// Total cost in USD
    #[serde(default)]
    pub usd: f64,

    /// Total input tokens
    #[serde(default)]
    pub input_tokens: u64,

    /// Total output tokens
    #[serde(default)]
    pub output_tokens: u64,
}

// ============================================================================
// Control Protocol - Incoming Requests
// ============================================================================

/// Incoming control request from CLI (hook callbacks, etc.)
/// This is sent by the CLI when it needs us to make a decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingControlRequest {
    /// Unique ID for this request (used to match responses)
    pub request_id: String,

    /// The request payload
    pub request: ControlRequestPayload,
}

/// Payload of an incoming control request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlRequestPayload {
    /// Subtype of the control request (e.g., "hook_callback")
    pub subtype: String,

    /// Callback ID from our hook registration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_id: Option<String>,

    /// Tool use ID (for tool-related hooks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,

    /// Hook input data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<HookCallbackInput>,
}

/// Input data for a hook callback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookCallbackInput {
    /// Hook event name (e.g., "PreToolUse", "PostToolUse")
    pub hook_event_name: String,

    /// Tool name (for tool hooks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,

    /// Tool input parameters (for tool hooks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_input: Option<Value>,
}

/// Acknowledgment of a control response we sent.
///
/// The CLI has emitted this in two formats across versions:
/// 1. Flat: `{ "subtype": "...", "request_id": "..." }`
/// 2. Nested: `{ "response": { "subtype": "...", "request_id": "...", ... } }`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ControlResponseAck {
    /// Legacy flat acknowledgment payload
    Flat {
        /// Subtype (usually "success")
        subtype: String,
        /// Request ID this acknowledges
        request_id: String,
    },
    /// Current nested acknowledgment payload
    Nested {
        /// Nested response object
        response: ControlResponseAckPayload,
    },
}

/// Nested control response acknowledgment payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlResponseAckPayload {
    /// Subtype (usually "success")
    pub subtype: String,
    /// Request ID this acknowledges
    pub request_id: String,
}

// ============================================================================
// Control Protocol - Outgoing Responses
// ============================================================================

/// Outgoing control response we send to the CLI
#[derive(Debug, Clone, Serialize)]
pub struct OutgoingControlResponse {
    /// Always "control_response"
    #[serde(rename = "type")]
    pub msg_type: String,

    /// The response payload
    pub response: OutgoingResponsePayload,
}

/// Payload of our outgoing control response
#[derive(Debug, Clone, Serialize)]
pub struct OutgoingResponsePayload {
    /// Subtype (usually "success")
    pub subtype: String,

    /// Request ID this is responding to
    pub request_id: String,

    /// Hook-specific response data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<HookResponseData>,
}

/// Hook-specific response data
#[derive(Debug, Clone, Serialize)]
pub struct HookResponseData {
    /// Whether to continue execution
    #[serde(rename = "continue")]
    pub continue_: bool,

    /// Hook-specific output
    #[serde(rename = "hookSpecificOutput")]
    pub hook_specific_output: OutgoingHookSpecificOutput,
}

/// Hook-specific output for outgoing responses
#[derive(Debug, Clone, Serialize)]
pub struct OutgoingHookSpecificOutput {
    /// Hook event name (e.g., "PreToolUse")
    #[serde(rename = "hookEventName")]
    pub hook_event_name: String,

    /// Permission decision (e.g., "allow", "deny")
    #[serde(rename = "permissionDecision")]
    pub permission_decision: String,

    /// Reason for the decision
    #[serde(
        rename = "permissionDecisionReason",
        skip_serializing_if = "Option::is_none"
    )]
    pub permission_decision_reason: Option<String>,

    /// Modified tool input (if any)
    #[serde(rename = "updatedInput", skip_serializing_if = "Option::is_none")]
    pub updated_input: Option<Value>,
}

impl OutgoingControlResponse {
    /// Create an allow response for a PreToolUse hook callback
    pub fn allow_pretool(request_id: String, reason: &str) -> Self {
        Self {
            msg_type: "control_response".to_string(),
            response: OutgoingResponsePayload {
                subtype: "success".to_string(),
                request_id,
                response: Some(HookResponseData {
                    continue_: true,
                    hook_specific_output: OutgoingHookSpecificOutput {
                        hook_event_name: "PreToolUse".to_string(),
                        permission_decision: "allow".to_string(),
                        permission_decision_reason: Some(reason.to_string()),
                        updated_input: None,
                    },
                }),
            },
        }
    }

    /// Create a deny response for a PreToolUse hook callback
    pub fn deny_pretool(request_id: String, reason: &str) -> Self {
        Self {
            msg_type: "control_response".to_string(),
            response: OutgoingResponsePayload {
                subtype: "success".to_string(),
                request_id,
                response: Some(HookResponseData {
                    continue_: true,
                    hook_specific_output: OutgoingHookSpecificOutput {
                        hook_event_name: "PreToolUse".to_string(),
                        permission_decision: "deny".to_string(),
                        permission_decision_reason: Some(reason.to_string()),
                        updated_input: None,
                    },
                }),
            },
        }
    }

    /// Create an acknowledgment response for a PostToolUse hook
    pub fn ack_posttool(request_id: String) -> Self {
        Self {
            msg_type: "control_response".to_string(),
            response: OutgoingResponsePayload {
                subtype: "success".to_string(),
                request_id,
                response: Some(HookResponseData {
                    continue_: true,
                    hook_specific_output: OutgoingHookSpecificOutput {
                        hook_event_name: "PostToolUse".to_string(),
                        permission_decision: "allow".to_string(),
                        permission_decision_reason: None,
                        updated_input: None,
                    },
                }),
            },
        }
    }

    /// Create an acknowledgment for the initialize response
    pub fn ack_initialize(request_id: String) -> Self {
        Self {
            msg_type: "control_response".to_string(),
            response: OutgoingResponsePayload {
                subtype: "success".to_string(),
                request_id,
                response: None,
            },
        }
    }
}

// ============================================================================
// Control Protocol - Legacy Types (for reference)
// ============================================================================

/// Legacy control request structure (for reference/compatibility)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlRequest {
    /// Always "control_request" for these messages
    #[serde(rename = "type")]
    pub request_type: String,

    /// Subtype of the control request
    pub subtype: ControlRequestSubtype,

    /// Hook type that triggered this request
    pub hook_type: HookType,

    /// Unique ID for this request (used to match responses)
    pub request_id: String,

    /// Session ID for context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    /// Tool use ID (for tool-related hooks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,

    /// Name of the tool (for tool-related hooks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,

    /// Tool input parameters (for tool-related hooks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_input: Option<Value>,
}

/// Subtypes for control requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlRequestSubtype {
    /// Request for a hook callback
    HookCallback,
}

/// Types of hooks that can trigger control requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookType {
    /// Before tool execution - can approve/deny/modify
    PreToolUse,
    /// After tool execution - can observe results
    PostToolUse,
    /// When a notification is generated
    Notification,
    /// Before stopping the turn
    Stop,
}

// ============================================================================
// Control Protocol - Responses
// ============================================================================

/// Control response sent to the CLI for permission decisions.
///
/// The client sends this in response to a `ControlRequest`.
/// It must include the matching `request_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlResponse {
    /// Always "control_response" for these messages
    #[serde(rename = "type")]
    pub response_type: String,

    /// Request ID this response is for (must match the request)
    pub request_id: String,

    /// Hook-specific output data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook_specific_output: Option<HookSpecificOutput>,

    /// Whether to abort the current operation
    #[serde(default)]
    pub abort: bool,

    /// Error message if something went wrong
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ControlResponse {
    /// Create a new control response for the given request ID.
    pub fn new(request_id: String) -> Self {
        Self {
            response_type: "control_response".to_string(),
            request_id,
            hook_specific_output: None,
            abort: false,
            error: None,
        }
    }

    /// Create an allow response for a PreToolUse hook.
    pub fn allow(request_id: String, reason: &str) -> Self {
        Self {
            response_type: "control_response".to_string(),
            request_id,
            hook_specific_output: Some(HookSpecificOutput::PreToolUse(PreToolUseOutput {
                permission_decision: Some(CliPermissionDecision::Allow),
                permission_decision_reason: Some(reason.to_string()),
                updated_input: None,
            })),
            abort: false,
            error: None,
        }
    }

    /// Create a deny response for a PreToolUse hook.
    pub fn deny(request_id: String, reason: &str) -> Self {
        Self {
            response_type: "control_response".to_string(),
            request_id,
            hook_specific_output: Some(HookSpecificOutput::PreToolUse(PreToolUseOutput {
                permission_decision: Some(CliPermissionDecision::Deny),
                permission_decision_reason: Some(reason.to_string()),
                updated_input: None,
            })),
            abort: false,
            error: None,
        }
    }
}

/// Hook-specific output data in control responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "hook_type", rename_all = "snake_case")]
pub enum HookSpecificOutput {
    /// Output for PreToolUse hooks
    PreToolUse(PreToolUseOutput),
    /// Output for PostToolUse hooks
    PostToolUse(PostToolUseOutput),
}

/// Output for PreToolUse hook responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreToolUseOutput {
    /// Permission decision: allow or deny
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_decision: Option<CliPermissionDecision>,

    /// Reason for the permission decision
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_decision_reason: Option<String>,

    /// Modified tool input (if the hook wants to change parameters)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_input: Option<Value>,
}

/// Output for PostToolUse hook responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostToolUseOutput {
    /// Modified tool result (if the hook wants to change the result)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_result: Option<Value>,
}

/// Permission decision for tool use (serialized over the CLI protocol).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliPermissionDecision {
    /// Allow the tool to execute
    Allow,
    /// Deny the tool execution
    Deny,
}

// ============================================================================
// Tool Result Helper
// ============================================================================

/// Helper struct for constructing tool results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// ID of the tool use this result corresponds to
    pub tool_use_id: String,

    /// Result content (can be string or structured)
    pub content: Value,

    /// Whether the tool execution resulted in an error
    #[serde(default)]
    pub is_error: bool,
}

impl ToolResult {
    /// Create a successful tool result.
    pub fn success(tool_use_id: String, content: impl Into<Value>) -> Self {
        Self {
            tool_use_id,
            content: content.into(),
            is_error: false,
        }
    }

    /// Create an error tool result.
    pub fn error(tool_use_id: String, error_message: impl Into<Value>) -> Self {
        Self {
            tool_use_id,
            content: error_message.into(),
            is_error: true,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_deserialize_system_event() {
        let json = r#"{"type":"system","subtype":"init","session_id":"abc123"}"#;
        let event: CliEvent = serde_json::from_str(json).unwrap();

        if let CliEvent::System(sys) = event {
            assert_eq!(sys.subtype, "init");
            assert_eq!(sys.session_id, Some("abc123".to_string()));
        } else {
            panic!("Expected System event");
        }
    }

    #[test]
    fn test_deserialize_assistant_text() {
        let json = r#"{
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [{"type": "text", "text": "Hello!"}],
                "model": "claude-sonnet-4-5"
            }
        }"#;

        let event: CliEvent = serde_json::from_str(json).unwrap();

        if let CliEvent::Assistant(assistant) = event {
            assert_eq!(assistant.message.role, "assistant");
            assert_eq!(assistant.message.content.len(), 1);
            if let ContentBlock::Text(text) = &assistant.message.content[0] {
                assert_eq!(text.text, "Hello!");
            } else {
                panic!("Expected Text block");
            }
        } else {
            panic!("Expected Assistant event");
        }
    }

    #[test]
    fn test_deserialize_tool_use() {
        let json = r#"{
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": "tool123",
                    "name": "Read",
                    "input": {"file_path": "/test.rs"}
                }]
            }
        }"#;

        let event: CliEvent = serde_json::from_str(json).unwrap();

        if let CliEvent::Assistant(assistant) = event {
            if let ContentBlock::ToolUse(tool) = &assistant.message.content[0] {
                assert_eq!(tool.id, "tool123");
                assert_eq!(tool.name, "Read");
                assert_eq!(tool.input["file_path"], "/test.rs");
            } else {
                panic!("Expected ToolUse block");
            }
        } else {
            panic!("Expected Assistant event");
        }
    }

    #[test]
    fn test_deserialize_thinking() {
        let json = r#"{
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [{
                    "type": "thinking",
                    "thinking": "Let me analyze this..."
                }]
            }
        }"#;

        let event: CliEvent = serde_json::from_str(json).unwrap();

        if let CliEvent::Assistant(assistant) = event {
            if let ContentBlock::Thinking(thinking) = &assistant.message.content[0] {
                assert_eq!(thinking.thinking, "Let me analyze this...");
            } else {
                panic!("Expected Thinking block");
            }
        } else {
            panic!("Expected Assistant event");
        }
    }

    #[test]
    fn test_deserialize_result_success() {
        let json = r#"{
            "type": "result",
            "subtype": "success",
            "cost_usd": 0.001,
            "duration_ms": 5000,
            "session_id": "session123"
        }"#;

        let event: CliEvent = serde_json::from_str(json).unwrap();

        if let CliEvent::Result(result) = event {
            assert_eq!(result.subtype, "success");
            assert_eq!(result.cost_usd, Some(0.001));
            assert_eq!(result.duration_ms, Some(5000));
            assert_eq!(result.session_id, Some("session123".to_string()));
        } else {
            panic!("Expected Result event");
        }
    }

    #[test]
    fn test_deserialize_control_response_ack_flat() {
        let json = r#"{
            "type": "control_response",
            "subtype": "success",
            "request_id": "req_123"
        }"#;

        let event: CliEvent = serde_json::from_str(json).unwrap();
        match event {
            CliEvent::ControlResponse(ControlResponseAck::Flat {
                subtype,
                request_id,
            }) => {
                assert_eq!(subtype, "success");
                assert_eq!(request_id, "req_123");
            }
            _ => panic!("Expected flat ControlResponse ack"),
        }
    }

    #[test]
    fn test_deserialize_control_response_ack_nested() {
        let json = r#"{
            "type": "control_response",
            "response": {
                "subtype": "success",
                "request_id": "req_456",
                "response": {
                    "commands": []
                }
            }
        }"#;

        let event: CliEvent = serde_json::from_str(json).unwrap();
        match event {
            CliEvent::ControlResponse(ControlResponseAck::Nested { response }) => {
                assert_eq!(response.subtype, "success");
                assert_eq!(response.request_id, "req_456");
            }
            _ => panic!("Expected nested ControlResponse ack"),
        }
    }

    #[test]
    fn test_serialize_control_response_allow() {
        let response = ControlResponse::allow("req123".to_string(), "User approved");

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["type"], "control_response");
        assert_eq!(json["request_id"], "req123");
        assert_eq!(json["hook_specific_output"]["permission_decision"], "allow");
        assert_eq!(
            json["hook_specific_output"]["permission_decision_reason"],
            "User approved"
        );
    }

    #[test]
    fn test_serialize_control_response_deny() {
        let response = ControlResponse::deny("req456".to_string(), "Security policy");

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["type"], "control_response");
        assert_eq!(json["request_id"], "req456");
        assert_eq!(json["hook_specific_output"]["permission_decision"], "deny");
    }

    #[test]
    fn test_tool_result_success() {
        let result = ToolResult::success("tool123".to_string(), "File contents here");

        assert_eq!(result.tool_use_id, "tool123");
        assert_eq!(result.content, json!("File contents here"));
        assert!(!result.is_error);
    }

    #[test]
    fn test_tool_result_error() {
        let result = ToolResult::error("tool456".to_string(), "File not found");

        assert_eq!(result.tool_use_id, "tool456");
        assert_eq!(result.content, json!("File not found"));
        assert!(result.is_error);
    }

    #[test]
    fn test_usage_info_defaults() {
        let json = r#"{}"#;
        let usage: UsageInfo = serde_json::from_str(json).unwrap();

        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.cache_read_input_tokens, 0);
        assert_eq!(usage.cache_creation_input_tokens, 0);
    }

    #[test]
    fn test_permission_decision_serialization() {
        assert_eq!(
            serde_json::to_string(&CliPermissionDecision::Allow).unwrap(),
            r#""allow""#
        );
        assert_eq!(
            serde_json::to_string(&CliPermissionDecision::Deny).unwrap(),
            r#""deny""#
        );
    }

    #[test]
    fn test_deserialize_control_request() {
        let json = r#"{
            "type": "control_request",
            "request_id": "req_123",
            "request": {
                "subtype": "hook_callback",
                "callback_id": "pretool_0",
                "tool_use_id": "tool_456",
                "input": {
                    "hook_event_name": "PreToolUse",
                    "tool_name": "Bash",
                    "tool_input": {"command": "ls -la"}
                }
            }
        }"#;

        let event: CliEvent = serde_json::from_str(json).unwrap();

        if let CliEvent::ControlRequest(req) = event {
            assert_eq!(req.request_id, "req_123");
            assert_eq!(req.request.subtype, "hook_callback");
            assert_eq!(req.request.callback_id, Some("pretool_0".to_string()));
            assert_eq!(req.request.tool_use_id, Some("tool_456".to_string()));

            let input = req.request.input.unwrap();
            assert_eq!(input.hook_event_name, "PreToolUse");
            assert_eq!(input.tool_name, Some("Bash".to_string()));
        } else {
            panic!("Expected ControlRequest event");
        }
    }

    #[test]
    fn test_serialize_outgoing_control_response() {
        let response =
            OutgoingControlResponse::allow_pretool("req_123".to_string(), "User approved");

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["type"], "control_response");
        assert_eq!(json["response"]["subtype"], "success");
        assert_eq!(json["response"]["request_id"], "req_123");
        assert_eq!(json["response"]["response"]["continue"], true);
        assert_eq!(
            json["response"]["response"]["hookSpecificOutput"]["hookEventName"],
            "PreToolUse"
        );
        assert_eq!(
            json["response"]["response"]["hookSpecificOutput"]["permissionDecision"],
            "allow"
        );
        assert_eq!(
            json["response"]["response"]["hookSpecificOutput"]["permissionDecisionReason"],
            "User approved"
        );
    }

    #[test]
    fn test_serialize_deny_control_response() {
        let response =
            OutgoingControlResponse::deny_pretool("req_456".to_string(), "Security policy");

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(
            json["response"]["response"]["hookSpecificOutput"]["permissionDecision"],
            "deny"
        );
        assert_eq!(
            json["response"]["response"]["hookSpecificOutput"]["permissionDecisionReason"],
            "Security policy"
        );
    }
}
