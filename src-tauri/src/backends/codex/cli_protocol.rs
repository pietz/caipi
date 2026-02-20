use serde::Serialize;
use serde_json::Value;
pub(crate) use super::tool_utils::normalized_tool_from_item;

// ---------------------------------------------------------------------------
// Helpers (retained from previous implementation)
// ---------------------------------------------------------------------------

pub fn event_type(value: &Value) -> Option<&str> {
    value.get("type").and_then(Value::as_str)
}

pub fn first_string<'a>(value: &'a Value, paths: &[&[&str]]) -> Option<&'a str> {
    for path in paths {
        let mut current = value;
        let mut valid = true;
        for key in *path {
            match current.get(*key) {
                Some(next) => current = next,
                None => {
                    valid = false;
                    break;
                }
            }
        }
        if valid {
            if let Some(s) = current.as_str() {
                return Some(s);
            }
        }
    }
    None
}

pub fn clean_thinking_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.len() >= 4 && trimmed.starts_with("**") && trimmed.ends_with("**") {
        trimmed[2..trimmed.len() - 2].trim().to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn final_tool_status(tool_type: &str, item_status: &str, exit_code: Option<i64>) -> &'static str {
    if item_status != "completed" {
        return "error";
    }
    if tool_type == "command_execution" {
        if exit_code == Some(0) {
            "completed"
        } else {
            "error"
        }
    } else {
        "completed"
    }
}

// ---------------------------------------------------------------------------
// JSON-RPC message types for `codex app-server`
// ---------------------------------------------------------------------------

/// Classify an incoming JSONL line from the app-server stdout.
#[derive(Debug)]
#[allow(dead_code)]
pub enum IncomingMessage {
    /// A response to one of our requests (has `id` + `result` or `error`).
    Response {
        id: u64,
        result: Option<Value>,
        error: Option<Value>,
    },
    /// A server-initiated notification (has `method`, no `id`).
    Notification { method: String, params: Value },
    /// A server-initiated request that expects a response (has `method` + `id`, no `result`).
    Request {
        id: Value,
        method: String,
        params: Value,
    },
}

impl IncomingMessage {
    /// Parse a JSON value into an `IncomingMessage`.
    pub fn parse(value: &Value) -> Option<Self> {
        let has_id = value.get("id").is_some();
        let has_method = value.get("method").is_some();
        let has_result = value.get("result").is_some();
        let has_error = value.get("error").is_some();

        if has_id && (has_result || has_error) {
            // Response to our request
            let id = value.get("id")?.as_u64()?;
            return Some(IncomingMessage::Response {
                id,
                result: value.get("result").cloned(),
                error: value.get("error").cloned(),
            });
        }

        if has_method && has_id && !has_result {
            // Server asking us something (approval request)
            return Some(IncomingMessage::Request {
                id: value.get("id")?.clone(),
                method: value.get("method")?.as_str()?.to_string(),
                params: value.get("params").cloned().unwrap_or(Value::Null),
            });
        }

        if has_method && !has_id {
            // Server notification
            return Some(IncomingMessage::Notification {
                method: value.get("method")?.as_str()?.to_string(),
                params: value.get("params").cloned().unwrap_or(Value::Null),
            });
        }

        None
    }
}

// ---------------------------------------------------------------------------
// Outgoing JSON-RPC types
// ---------------------------------------------------------------------------

/// A JSON-RPC request we send to the server (expects a response).
#[derive(Debug, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: &'static str,
    pub method: String,
    pub id: u64,
    pub params: Value,
}

impl JsonRpcRequest {
    pub fn new(method: impl Into<String>, id: u64, params: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            method: method.into(),
            id,
            params,
        }
    }
}

/// A JSON-RPC notification we send to the server (no response expected).
#[derive(Debug, Serialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: &'static str,
    pub method: String,
    pub params: Value,
}

impl JsonRpcNotification {
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC response to a server request (e.g. approval decision).
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    pub id: Value,
    pub result: Value,
}

impl JsonRpcResponse {
    pub fn new(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result,
        }
    }
}

// ---------------------------------------------------------------------------
// Approval helpers
// ---------------------------------------------------------------------------

/// Extract tool name and target from an approval request.
///
/// Returns `(tool_type, target)`.
pub fn extract_approval_tool_info(method: &str, params: &Value) -> (String, String) {
    match method {
        "item/commandExecution/requestApproval" => {
            let command = params
                .get("command")
                .and_then(Value::as_str)
                .or_else(|| {
                    params
                        .get("commandCall")
                        .and_then(|c| c.get("command"))
                        .and_then(Value::as_str)
                })
                .unwrap_or("shell command")
                .to_string();
            ("command_execution".to_string(), command)
        }
        "item/fileChange/requestApproval" => {
            let file_path = params
                .get("path")
                .or_else(|| params.get("filePath"))
                .and_then(Value::as_str)
                .unwrap_or("file")
                .to_string();
            ("file_change".to_string(), file_path)
        }
        _ => {
            let tool_type = method
                .strip_suffix("/requestApproval")
                .unwrap_or(method)
                .rsplit('/')
                .next()
                .unwrap_or("unknown")
                .to_string();
            ("tool_use".to_string(), tool_type)
        }
    }
}

// ---------------------------------------------------------------------------
// Token usage extraction
// ---------------------------------------------------------------------------

/// Extract token usage from `thread/tokenUsage/updated` notification params
/// or from `turn/completed` params (legacy).
pub fn token_usage_from_turn_completed(params: &Value) -> Option<(u64, Option<u64>, Option<u64>)> {
    // thread/tokenUsage/updated: params.tokenUsage.total.totalTokens (camelCase)
    // Also try params.usage and params directly as fallbacks.
    let usage = params
        .get("tokenUsage")
        .and_then(|tu| tu.get("total"))
        .or_else(|| params.get("usage"))
        .unwrap_or(params);

    let total = usage
        .get("totalTokens")
        .or_else(|| usage.get("total_tokens"))
        .and_then(Value::as_u64)
        .or_else(|| {
            let input = usage.get("inputTokens").or_else(|| usage.get("input_tokens")).and_then(Value::as_u64).unwrap_or(0);
            let output = usage.get("outputTokens").or_else(|| usage.get("output_tokens")).and_then(Value::as_u64).unwrap_or(0);
            let sum = input + output;
            if sum > 0 { Some(sum) } else { None }
        })?;

    let context_tokens = usage
        .get("inputTokens")
        .or_else(|| usage.get("input_tokens"))
        .and_then(Value::as_u64);

    let context_window = params
        .get("tokenUsage")
        .and_then(|tu| tu.get("modelContextWindow"))
        .or_else(|| usage.get("model_context_window"))
        .and_then(Value::as_u64);

    Some((total, context_tokens, context_window))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn clean_thinking_text_strips_wrapping_bold_markers() {
        assert_eq!(
            clean_thinking_text("**Planning command execution updates**"),
            "Planning command execution updates"
        );
    }

    #[test]
    fn clean_thinking_text_keeps_normal_text() {
        assert_eq!(clean_thinking_text("Thinking..."), "Thinking...");
    }

    #[test]
    fn final_tool_status_for_command_requires_zero_exit_code() {
        assert_eq!(final_tool_status("command_execution", "completed", Some(0)), "completed");
        assert_eq!(final_tool_status("command_execution", "completed", Some(1)), "error");
        assert_eq!(final_tool_status("command_execution", "completed", None), "error");
    }

    #[test]
    fn final_tool_status_for_non_command_uses_item_status() {
        assert_eq!(final_tool_status("web_search", "completed", None), "completed");
        assert_eq!(final_tool_status("web_search", "failed", None), "error");
    }

    #[test]
    fn normalized_tool_from_item_maps_web_run_search_to_web_search() {
        let item = json!({
            "type": "function_call",
            "name": "web.run",
            "arguments": "{\"search_query\":[{\"q\":\"latest rust release\"}]}"
        });
        let (tool_type, target, input) = normalized_tool_from_item(&item);
        assert_eq!(tool_type, "web_search");
        assert_eq!(target, "latest rust release");
        assert!(input.is_some());
    }

    #[test]
    fn normalized_tool_from_item_maps_web_run_open_to_web_fetch() {
        let item = json!({
            "type": "function_call",
            "name": "web.run",
            "arguments": "{\"open\":[{\"ref_id\":\"turn0search0\"}]}"
        });
        let (tool_type, target, input) = normalized_tool_from_item(&item);
        assert_eq!(tool_type, "web_fetch");
        assert_eq!(target, "turn0search0");
        assert!(input.is_some());
    }

    #[test]
    fn parse_incoming_response() {
        let value = json!({"id": 1, "result": {"threadId": "abc"}, "jsonrpc": "2.0"});
        match IncomingMessage::parse(&value) {
            Some(IncomingMessage::Response { id, result, error }) => {
                assert_eq!(id, 1);
                assert!(result.is_some());
                assert!(error.is_none());
            }
            other => panic!("Expected Response, got {:?}", other),
        }
    }

    #[test]
    fn parse_incoming_notification() {
        let value = json!({"method": "turn/started", "params": {"turnId": "t1"}, "jsonrpc": "2.0"});
        match IncomingMessage::parse(&value) {
            Some(IncomingMessage::Notification { method, params }) => {
                assert_eq!(method, "turn/started");
                assert_eq!(params.get("turnId").and_then(Value::as_str), Some("t1"));
            }
            other => panic!("Expected Notification, got {:?}", other),
        }
    }

    #[test]
    fn parse_incoming_request() {
        let value = json!({
            "id": "req_1",
            "method": "item/commandExecution/requestApproval",
            "params": {"parsedCmd": ["ls", "-la"]},
            "jsonrpc": "2.0"
        });
        match IncomingMessage::parse(&value) {
            Some(IncomingMessage::Request { id, method, params }) => {
                assert_eq!(id, json!("req_1"));
                assert_eq!(method, "item/commandExecution/requestApproval");
                assert!(params.get("parsedCmd").is_some());
            }
            other => panic!("Expected Request, got {:?}", other),
        }
    }

    #[test]
    fn extract_command_approval_info() {
        let params = json!({"command": "git push"});
        let (tool_type, target) =
            extract_approval_tool_info("item/commandExecution/requestApproval", &params);
        assert_eq!(tool_type, "command_execution");
        assert_eq!(target, "git push");
    }

    #[test]
    fn extract_file_approval_info() {
        let params = json!({"path": "/src/main.rs"});
        let (tool_type, target) =
            extract_approval_tool_info("item/fileChange/requestApproval", &params);
        assert_eq!(tool_type, "file_change");
        assert_eq!(target, "/src/main.rs");
    }

    #[test]
    fn token_usage_from_completed_event() {
        let params = json!({
            "usage": {
                "input_tokens": 1000,
                "output_tokens": 200,
                "total_tokens": 1200,
                "model_context_window": 128000
            }
        });
        let (total, ctx, window) = token_usage_from_turn_completed(&params).unwrap();
        assert_eq!(total, 1200);
        assert_eq!(ctx, Some(1000));
        assert_eq!(window, Some(128000));
    }
}
