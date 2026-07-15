use serde_json::{Map, Value};

fn parse_arguments(value: &Value) -> Option<Value> {
    let arguments = value.get("arguments")?;
    if arguments.is_object() || arguments.is_array() {
        return Some(arguments.clone());
    }
    arguments
        .as_str()
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
}

fn normalize_collab_tool_name(value: &str) -> String {
    match value {
        "spawnAgent" | "spawn_agent" => "spawn_agent".to_string(),
        "sendInput" | "send_input" => "send_input".to_string(),
        "resumeAgent" | "resume_agent" => "resume_agent".to_string(),
        "closeAgent" | "close_agent" => "close_agent".to_string(),
        "wait" => "wait".to_string(),
        other => other.to_string(),
    }
}

fn normalize_tool_type(value: &str) -> String {
    match value {
        "commandExecution" => "command_execution".to_string(),
        "fileChange" => "file_change".to_string(),
        "webSearch" => "web_search".to_string(),
        "webFetch" => "web_fetch".to_string(),
        other => other.to_string(),
    }
}

fn target_from_args(args: &Value) -> Option<String> {
    args.get("cmd")
        .or_else(|| args.get("prompt"))
        .or_else(|| args.get("message"))
        .or_else(|| args.get("query"))
        .or_else(|| args.get("command"))
        .or_else(|| args.get("id"))
        .or_else(|| args.get("task"))
        .or_else(|| args.get("description"))
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .or_else(|| {
            args.get("ids")
                .and_then(Value::as_array)
                .and_then(|arr| arr.first())
                .and_then(Value::as_str)
                .map(|s| s.to_string())
        })
}

fn merge_input_with_metadata(
    input: Option<Value>,
    item: &Value,
    metadata_keys: &[&str],
) -> Option<Value> {
    let mut map = match input {
        Some(Value::Object(obj)) => obj,
        Some(other) => {
            let mut obj = Map::new();
            obj.insert("arguments".to_string(), other);
            obj
        }
        None => Map::new(),
    };

    for key in metadata_keys {
        if let Some(value) = item.get(*key) {
            map.entry((*key).to_string()).or_insert_with(|| value.clone());
        }
    }

    if map.is_empty() {
        None
    } else {
        Some(Value::Object(map))
    }
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

pub(crate) fn normalized_tool_from_item(item: &Value) -> (String, String, Option<Value>) {
    let raw_tool_type = item
        .get("type")
        .and_then(Value::as_str)
        .or_else(|| item.get("name").and_then(Value::as_str))
        .unwrap_or("command_execution");

    // Codex 0.105+ sub-agent protocol.
    if raw_tool_type == "collabAgentToolCall" || raw_tool_type == "collab_agent_tool_call" {
        let tool = item.get("tool").and_then(Value::as_str).unwrap_or(raw_tool_type);
        let tool_type = normalize_collab_tool_name(tool);
        let input = merge_input_with_metadata(
            parse_arguments(item),
            item,
            &[
                "tool",
                "prompt",
                "message",
                "id",
                "ids",
                "senderThreadId",
                "sender_thread_id",
                "receiverThreadIds",
                "receiver_thread_ids",
                "agentsStates",
                "status",
            ],
        );
        let target = input
            .as_ref()
            .and_then(target_from_args)
            .or_else(|| {
                item.get("prompt")
                    .or_else(|| item.get("message"))
                    .and_then(Value::as_str)
                    .map(|s| s.to_string())
            })
            .unwrap_or_default();
        return (tool_type, target, input);
    }

    // Codex 0.104.x compatibility path.
    if raw_tool_type == "function_call" {
        let function_name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("command_execution");
        let arguments = merge_input_with_metadata(
            parse_arguments(item),
            item,
            &[
                "threadId",
                "thread_id",
                "senderThreadId",
                "sender_thread_id",
                "receiverThreadIds",
                "receiver_thread_ids",
            ],
        );

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
            "exec_command" => "command_execution".to_string(),
            other => normalize_tool_type(other),
        };
        let target = arguments
            .as_ref()
            .and_then(target_from_args)
            .unwrap_or_default();
        return (tool_type, target, arguments);
    }

    // Some Codex builds emit collab tools directly as raw item types.
    if matches!(
        raw_tool_type,
        "spawn_agent"
            | "spawnAgent"
            | "send_input"
            | "sendInput"
            | "resume_agent"
            | "resumeAgent"
            | "close_agent"
            | "closeAgent"
            | "wait"
    ) {
        let tool_type = normalize_collab_tool_name(raw_tool_type);
        let input = merge_input_with_metadata(
            parse_arguments(item),
            item,
            &[
                "prompt",
                "message",
                "id",
                "ids",
                "senderThreadId",
                "sender_thread_id",
                "receiverThreadIds",
                "receiver_thread_ids",
                "agentsStates",
                "status",
            ],
        );
        let target = input
            .as_ref()
            .and_then(target_from_args)
            .or_else(|| {
                item.get("prompt")
                    .or_else(|| item.get("message"))
                    .and_then(Value::as_str)
                    .map(std::string::ToString::to_string)
            })
            .unwrap_or_default();
        return (tool_type, target, input);
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

    if raw_tool_type == "web_search" || raw_tool_type == "web_fetch" {
        let target = item
            .get("action")
            .and_then(|action| action.get("query").or_else(|| action.get("url")))
            .and_then(Value::as_str)
            .map(std::string::ToString::to_string)
            .or_else(|| {
                merge_input_with_metadata(parse_arguments(item), item, &[])
                    .as_ref()
                    .map(web_run_target_from_args)
            })
            .unwrap_or_default();
        return (raw_tool_type.to_string(), target, None);
    }

    let target = item
        .get("command")
        .or_else(|| item.pointer("/commandCall/command"))
        .or_else(|| item.get("commandCall").and_then(|call| call.get("command")))
        .or_else(|| item.get("cmd"))
        .or_else(|| item.get("query"))
        .or_else(|| item.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    (normalize_tool_type(raw_tool_type), target, None)
}

pub(crate) fn codex_tool_from_payload(payload: &Value) -> Option<(String, String)> {
    match payload.get("type").and_then(Value::as_str) {
        Some("function_call")
        | Some("web_search_call")
        | Some("collabAgentToolCall")
        | Some("collab_agent_tool_call") => {
            let (tool_type, target, _) = normalized_tool_from_item(payload);
            Some((tool_type, target))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{codex_tool_from_payload, normalized_tool_from_item};
    use serde_json::json;

    #[test]
    fn normalized_tool_maps_web_run_search_to_web_search() {
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
    fn normalized_tool_maps_web_run_open_to_web_fetch() {
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
    fn payload_tool_returns_none_for_non_tool_payload() {
        let payload = json!({
            "type": "text",
            "content": "hello"
        });
        assert!(codex_tool_from_payload(&payload).is_none());
    }

    #[test]
    fn normalized_tool_maps_collab_spawn_agent() {
        let item = json!({
            "type": "collabAgentToolCall",
            "tool": "spawnAgent",
            "prompt": "Research changelog",
            "senderThreadId": "thread-parent",
            "receiverThreadIds": ["thread-child"]
        });
        let (tool_type, target, input) = normalized_tool_from_item(&item);
        assert_eq!(tool_type, "spawn_agent");
        assert_eq!(target, "Research changelog");
        assert_eq!(
            input.and_then(|value| value.get("receiverThreadIds").cloned()),
            Some(json!(["thread-child"]))
        );
    }

    #[test]
    fn normalized_tool_preserves_snake_case_thread_metadata() {
        let item = json!({
            "type": "function_call",
            "name": "spawn_agent",
            "receiver_thread_ids": ["thread-child"],
            "sender_thread_id": "thread-parent",
            "arguments": {
                "message": "Investigate logs"
            }
        });
        let (_tool_type, _target, input) = normalized_tool_from_item(&item);
        assert_eq!(
            input
                .as_ref()
                .and_then(|value| value.get("receiver_thread_ids").cloned()),
            Some(json!(["thread-child"]))
        );
        assert_eq!(
            input
                .as_ref()
                .and_then(|value| value.get("sender_thread_id").cloned()),
            Some(json!("thread-parent"))
        );
    }

    #[test]
    fn payload_tool_maps_collab_wait() {
        let payload = json!({
            "type": "collabAgentToolCall",
            "tool": "wait",
            "ids": ["thread-child"]
        });
        let result = codex_tool_from_payload(&payload);
        assert_eq!(result, Some(("wait".to_string(), "thread-child".to_string())));
    }

    #[test]
    fn merge_metadata_does_not_override_existing_arguments() {
        let item = json!({
            "type": "function_call",
            "name": "send_input",
            "id": "tool-call-id",
            "message": "top-level message",
            "arguments": {
                "id": "agent-from-args",
                "message": "message from args"
            }
        });
        let (_tool_type, target, input) = normalized_tool_from_item(&item);
        assert_eq!(target, "message from args");
        assert_eq!(
            input.as_ref().and_then(|value| value.get("id").cloned()),
            Some(json!("agent-from-args"))
        );
        assert_eq!(
            input.as_ref().and_then(|value| value.get("message").cloned()),
            Some(json!("message from args"))
        );
    }

    #[test]
    fn normalized_tool_maps_camel_case_command_execution() {
        let item = json!({
            "type": "function_call",
            "name": "commandExecution",
            "arguments": { "cmd": "echo hi" }
        });
        let (tool_type, target, _) = normalized_tool_from_item(&item);
        assert_eq!(tool_type, "command_execution");
        assert_eq!(target, "echo hi");
    }

    #[test]
    fn normalized_raw_camel_case_item_type() {
        let item = json!({
            "type": "commandExecution",
            "command": "ls -la"
        });
        let (tool_type, target, _) = normalized_tool_from_item(&item);
        assert_eq!(tool_type, "command_execution");
        assert_eq!(target, "ls -la");
    }

    #[test]
    fn normalized_raw_wait_uses_ids_as_target_and_input() {
        let item = json!({
            "type": "wait",
            "ids": ["thread-child-1"]
        });
        let (tool_type, target, input) = normalized_tool_from_item(&item);
        assert_eq!(tool_type, "wait");
        assert_eq!(target, "thread-child-1");
        assert_eq!(
            input.and_then(|value| value.get("ids").cloned()),
            Some(json!(["thread-child-1"]))
        );
    }

    #[test]
    fn normalized_raw_web_search_reads_action_query() {
        let item = json!({
            "type": "web_search",
            "action": { "query": "latest hacker news" }
        });
        let (tool_type, target, _) = normalized_tool_from_item(&item);
        assert_eq!(tool_type, "web_search");
        assert_eq!(target, "latest hacker news");
    }
}
