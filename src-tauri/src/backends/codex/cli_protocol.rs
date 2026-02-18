use serde::Serialize;
use serde_json::Value;

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

fn parse_item_arguments(item: &Value) -> Option<Value> {
    let arguments = item.get("arguments")?;
    if arguments.is_object() || arguments.is_array() {
        return Some(arguments.clone());
    }
    arguments
        .as_str()
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
}

fn first_array_entry<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.get(key)?.as_array()?.first()
}

fn web_run_target_from_args(args: &Value) -> String {
    if let Some(query) = args
        .get("search_query")
        .and_then(Value::as_array)
        .and_then(|entries| entries.iter().find_map(|entry| entry.get("q")))
        .and_then(Value::as_str)
    {
        return query.to_string();
    }

    if let Some(query) = args
        .get("image_query")
        .and_then(Value::as_array)
        .and_then(|entries| entries.iter().find_map(|entry| entry.get("q")))
        .and_then(Value::as_str)
    {
        return query.to_string();
    }

    if let Some(reference) = first_array_entry(args, "open")
        .and_then(|entry| entry.get("ref_id"))
        .and_then(Value::as_str)
    {
        return reference.to_string();
    }

    if let Some(pattern) = first_array_entry(args, "find")
        .and_then(|entry| entry.get("pattern"))
        .and_then(Value::as_str)
    {
        return pattern.to_string();
    }

    if let Some(location) = first_array_entry(args, "weather")
        .and_then(|entry| entry.get("location"))
        .and_then(Value::as_str)
    {
        return location.to_string();
    }

    if let Some(ticker) = first_array_entry(args, "finance")
        .and_then(|entry| entry.get("ticker"))
        .and_then(Value::as_str)
    {
        return ticker.to_string();
    }

    if let Some(offset) = first_array_entry(args, "time")
        .and_then(|entry| entry.get("utc_offset"))
        .and_then(Value::as_str)
    {
        return offset.to_string();
    }

    if let Some(reference) = first_array_entry(args, "click")
        .and_then(|entry| entry.get("ref_id"))
        .and_then(Value::as_str)
    {
        return reference.to_string();
    }

    "web.run".to_string()
}

pub fn normalized_tool_from_item(item: &Value) -> (String, String, Option<Value>) {
    let raw_tool_type = item
        .get("type")
        .and_then(Value::as_str)
        .or_else(|| item.get("name").and_then(Value::as_str))
        .unwrap_or("command_execution");

    if raw_tool_type == "function_call" {
        let function_name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("command_execution");
        let arguments = parse_item_arguments(item);

        let target_from_args = arguments.as_ref().and_then(|args| {
            args.get("cmd")
                .or_else(|| args.get("query"))
                .or_else(|| args.get("command"))
                .or_else(|| args.get("task"))
                .or_else(|| args.get("prompt"))
                .or_else(|| args.get("description"))
                .and_then(Value::as_str)
                .map(|s| s.to_string())
        });

        if function_name == "web.run" {
            let has_search_queries = arguments
                .as_ref()
                .map(|args| {
                    args.get("search_query")
                        .or_else(|| args.get("image_query"))
                        .is_some()
                })
                .unwrap_or(false);
            let tool_type = if has_search_queries {
                "web_search"
            } else {
                "web_fetch"
            };
            let target = arguments
                .as_ref()
                .map(web_run_target_from_args)
                .unwrap_or_else(|| "web.run".to_string());
            return (tool_type.to_string(), target, arguments);
        }

        let tool_type = match function_name {
            "exec_command" => "command_execution",
            other => other,
        }
        .to_string();
        let target = target_from_args.unwrap_or_default();
        return (tool_type, target, arguments);
    }

    if raw_tool_type == "web_search_call" {
        let target = item
            .get("action")
            .and_then(|action| {
                action
                    .get("query")
                    .or_else(|| action.get("url"))
                    .and_then(Value::as_str)
            })
            .unwrap_or("")
            .to_string();
        return ("web_search".to_string(), target, None);
    }

    let target = item
        .get("command")
        .or_else(|| item.get("query"))
        .or_else(|| item.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    (raw_tool_type.to_string(), target, None)
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
                .get("commandCall")
                .and_then(|c| c.get("command"))
                .and_then(Value::as_str)
                .or_else(|| {
                    params
                        .get("parsedCmd")
                        .and_then(Value::as_array)
                        .and_then(|arr| arr.first())
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
// Token usage from turn/completed
// ---------------------------------------------------------------------------

pub fn token_usage_from_turn_completed(params: &Value) -> Option<(u64, Option<u64>, Option<u64>)> {
    // Try params.usage first, then params directly
    let usage = params.get("usage").unwrap_or(params);

    let total = usage
        .get("total_tokens")
        .and_then(Value::as_u64)
        .or_else(|| {
            let input = usage.get("input_tokens").and_then(Value::as_u64).unwrap_or(0);
            let output = usage.get("output_tokens").and_then(Value::as_u64).unwrap_or(0);
            let sum = input + output;
            if sum > 0 { Some(sum) } else { None }
        })?;

    let context_tokens = usage
        .get("input_tokens")
        .or_else(|| usage.get("context_tokens"))
        .and_then(Value::as_u64);

    let context_window = usage
        .get("model_context_window")
        .or_else(|| usage.get("context_window"))
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
        let params = json!({"parsedCmd": ["git", "push"]});
        let (tool_type, target) =
            extract_approval_tool_info("item/commandExecution/requestApproval", &params);
        assert_eq!(tool_type, "command_execution");
        assert_eq!(target, "git");
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
