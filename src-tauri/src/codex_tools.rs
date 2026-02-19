use serde_json::Value;

fn parse_arguments(value: &Value) -> Option<Value> {
    let arguments = value.get("arguments")?;
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

pub(crate) fn normalized_tool_from_item(item: &Value) -> (String, String, Option<Value>) {
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
        let arguments = parse_arguments(item);

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

pub(crate) fn codex_tool_from_payload(payload: &Value) -> Option<(String, String)> {
    match payload.get("type").and_then(Value::as_str) {
        Some("function_call") | Some("web_search_call") => {
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
}
