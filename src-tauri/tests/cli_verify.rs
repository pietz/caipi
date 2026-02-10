//! Integration tests for the Claude CLI ↔ Rust protocol boundary.
//!
//! These tests spawn the real `claude` CLI with Haiku, send controlled prompts,
//! and verify that every stdout line deserializes into our `CliEvent` types.
//!
//! All tests are `#[ignore]` — they require a working `claude` CLI and auth,
//! and they cost real API tokens.
//!
//! Run with:
//! ```bash
//! cd src-tauri && cargo test --test cli_verify -- --ignored --nocapture --test-threads=1
//! ```

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use caipi_lib::claude::cli_protocol::*;
use serde_json::Value;

/// Unique counter to give each test its own temp directory (they share a PID).
static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

// ============================================================================
// Test Harness
// ============================================================================

/// Whether to use hooks (permission callbacks) or bypass mode.
#[derive(Clone, Copy)]
enum HarnessMode {
    /// Use `--dangerously-skip-permissions` — no hook callbacks.
    Bypass,
    /// Register PreToolUse/PostToolUse hooks via initialize request.
    Hooks,
}

/// Result of parsing a single stdout line.
#[derive(Debug)]
enum ParsedLine {
    Ok(CliEvent),
    Err { line: String, error: String },
}

/// Simplified event type for sequence assertions.
#[derive(Debug, Clone, PartialEq, Eq)]
enum EventKind {
    System,
    Assistant,
    User,
    Result,
    PreToolUse,
    PostToolUse,
    ControlRequest,
    ControlResponse,
}

/// Harness that spawns `claude` CLI and collects events.
struct CliHarness {
    events: Vec<ParsedLine>,
}

impl CliHarness {
    /// Spawn the CLI, send a prompt, collect all events until Result or timeout.
    fn run(mode: HarnessMode, prompt: &str) -> Self {
        Self::run_with_timeout(mode, prompt, Duration::from_secs(120))
    }

    fn run_with_timeout(mode: HarnessMode, prompt: &str, timeout: Duration) -> Self {
        let test_id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp_dir = std::env::temp_dir().join(format!(
            "caipi_test_{}_{}", std::process::id(), test_id
        ));
        std::fs::create_dir_all(&tmp_dir).expect("create temp dir");

        let mut cmd = Command::new("claude");
        cmd.arg("-p")
            .arg("--output-format")
            .arg("stream-json")
            .arg("--verbose")
            .arg("--input-format")
            .arg("stream-json")
            .arg("--model")
            .arg("haiku");

        if matches!(mode, HarnessMode::Bypass) {
            cmd.arg("--dangerously-skip-permissions");
        }

        cmd.current_dir(&tmp_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().expect("Failed to spawn claude CLI — is it installed?");

        let mut stdin = child.stdin.take().expect("capture stdin");
        let stdout = child.stdout.take().expect("capture stdout");

        // Drain stderr in background thread to prevent deadlock
        let stderr = child.stderr.take().expect("capture stderr");
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(line) = line {
                    if !line.trim().is_empty() {
                        eprintln!("  [stderr] {}", line.trim());
                    }
                }
            }
        });

        // Send initialize request (with hooks if in Hooks mode)
        let init_request_id = "req_init_test";
        match mode {
            HarnessMode::Bypass => {
                // Even in bypass mode, send a minimal initialize
                let init = serde_json::json!({
                    "type": "control_request",
                    "request_id": init_request_id,
                    "request": {
                        "subtype": "initialize"
                    }
                });
                writeln!(stdin, "{}", serde_json::to_string(&init).unwrap()).unwrap();
            }
            HarnessMode::Hooks => {
                let init = serde_json::json!({
                    "type": "control_request",
                    "request_id": init_request_id,
                    "request": {
                        "subtype": "initialize",
                        "hooks": {
                            "PreToolUse": [{
                                "matcher": null,
                                "hookCallbackIds": ["pretool_0"]
                            }],
                            "PostToolUse": [{
                                "matcher": null,
                                "hookCallbackIds": ["posttool_0"]
                            }]
                        }
                    }
                });
                writeln!(stdin, "{}", serde_json::to_string(&init).unwrap()).unwrap();
            }
        }
        stdin.flush().unwrap();

        // Send user message
        let user_msg = serde_json::json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": prompt
            },
            "session_id": "default"
        });
        writeln!(stdin, "{}", serde_json::to_string(&user_msg).unwrap()).unwrap();
        stdin.flush().unwrap();

        // Read events from stdout via a channel so we can enforce a real timeout.
        // Without this, `reader.lines()` blocks forever if the CLI hangs.
        let (tx, rx) = mpsc::channel::<Result<String, String>>();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line_result in reader.lines() {
                match line_result {
                    Ok(line) => {
                        if tx.send(Ok(line)).is_err() {
                            break; // receiver dropped
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e.to_string()));
                        break;
                    }
                }
            }
        });

        let mut events = Vec::new();
        let start = Instant::now();
        let mut got_result = false;

        loop {
            let remaining = timeout.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                eprintln!("  [harness] TIMEOUT after {:?}", timeout);
                break;
            }

            match rx.recv_timeout(remaining) {
                Ok(Ok(line)) => {
                    if line.trim().is_empty() {
                        continue;
                    }

                    match serde_json::from_str::<CliEvent>(&line) {
                        Ok(event) => {
                            // Auto-respond to control requests (allow everything)
                            if let CliEvent::ControlRequest(ref req) = event {
                                let response = Self::auto_respond(req);
                                if let Some(resp) = response {
                                    let resp_json = serde_json::to_string(&resp).unwrap();
                                    let _ = writeln!(stdin, "{}", resp_json);
                                    let _ = stdin.flush();
                                }
                            }

                            if let CliEvent::Result(_) = &event {
                                got_result = true;
                            }

                            events.push(ParsedLine::Ok(event));

                            if got_result {
                                break;
                            }
                        }
                        Err(e) => {
                            eprintln!("  [harness] PARSE FAILURE: {}", e);
                            eprintln!("  [harness]   line: {}", &line[..line.len().min(200)]);
                            events.push(ParsedLine::Err {
                                line,
                                error: e.to_string(),
                            });
                        }
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("  [harness] read error: {}", e);
                    break;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    eprintln!("  [harness] TIMEOUT after {:?}", timeout);
                    break;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    eprintln!("  [harness] stdout closed unexpectedly");
                    break;
                }
            }
        }

        // Kill the process (don't wait for graceful shutdown in tests)
        let _ = child.kill();
        let _ = child.wait();

        // Clean up temp dir
        let _ = std::fs::remove_dir_all(&tmp_dir);

        CliHarness { events }
    }

    /// Auto-respond to a control request: allow PreToolUse, ack PostToolUse, ack initialize.
    fn auto_respond(req: &IncomingControlRequest) -> Option<OutgoingControlResponse> {
        let subtype = &req.request.subtype;
        if subtype == "initialize" {
            return Some(OutgoingControlResponse::ack_initialize(
                req.request_id.clone(),
            ));
        }
        if subtype == "hook_callback" {
            if let Some(ref input) = req.request.input {
                if input.hook_event_name == "PreToolUse" {
                    return Some(OutgoingControlResponse::allow_pretool(
                        req.request_id.clone(),
                        "test harness auto-allow",
                    ));
                }
                if input.hook_event_name == "PostToolUse" {
                    return Some(OutgoingControlResponse::ack_posttool(
                        req.request_id.clone(),
                    ));
                }
            }
        }
        None
    }

    // ========================================================================
    // Assertion helpers
    // ========================================================================

    /// All successfully parsed events.
    fn parsed_events(&self) -> Vec<&CliEvent> {
        self.events
            .iter()
            .filter_map(|e| match e {
                ParsedLine::Ok(ev) => Some(ev),
                _ => None,
            })
            .collect()
    }

    /// Lines that failed to deserialize.
    fn parse_failures(&self) -> Vec<(&str, &str)> {
        self.events
            .iter()
            .filter_map(|e| match e {
                ParsedLine::Err { line, error } => Some((line.as_str(), error.as_str())),
                _ => None,
            })
            .collect()
    }

    /// Check that a System init event is present.
    fn has_system_init(&self) -> bool {
        self.parsed_events().iter().any(|e| {
            matches!(e, CliEvent::System(sys) if sys.subtype == "init")
        })
    }

    /// Check that at least one text ContentBlock exists.
    fn has_text_content(&self) -> bool {
        self.parsed_events().iter().any(|e| {
            if let CliEvent::Assistant(a) = e {
                a.message
                    .content
                    .iter()
                    .any(|b| matches!(b, ContentBlock::Text(_)))
            } else {
                false
            }
        })
    }

    /// Check that a Result event with subtype "success" exists.
    fn has_result_success(&self) -> bool {
        self.parsed_events().iter().any(|e| {
            matches!(e, CliEvent::Result(r) if r.subtype == "success")
        })
    }

    /// Check that at least one assistant message has usage info.
    fn has_usage_info(&self) -> bool {
        self.parsed_events().iter().any(|e| {
            if let CliEvent::Assistant(a) = e {
                a.message.usage.is_some()
            } else {
                false
            }
        })
    }

    /// Check that a tool_use ContentBlock with the given name exists.
    fn has_tool_use(&self, name: &str) -> bool {
        self.tool_use_names().iter().any(|n| n == name)
    }

    /// All tool names seen in tool_use ContentBlocks.
    fn tool_use_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for e in self.parsed_events() {
            if let CliEvent::Assistant(a) = e {
                for block in &a.message.content {
                    if let ContentBlock::ToolUse(tu) = block {
                        names.push(tu.name.clone());
                    }
                }
            }
        }
        names
    }

    /// Check that a ControlRequest with hook_callback for a specific hook event arrived.
    fn has_hook_callback(&self, hook_name: &str) -> bool {
        self.parsed_events().iter().any(|e| {
            if let CliEvent::ControlRequest(req) = e {
                req.request.subtype == "hook_callback"
                    && req
                        .request
                        .input
                        .as_ref()
                        .map(|i| i.hook_event_name == hook_name)
                        .unwrap_or(false)
            } else {
                false
            }
        })
    }

    /// Get the Result event (if any).
    fn result_event(&self) -> Option<&ResultEvent> {
        self.parsed_events().iter().find_map(|e| {
            if let CliEvent::Result(r) = e {
                Some(r)
            } else {
                None
            }
        })
    }

    /// Get the System init event (if any).
    fn system_init_event(&self) -> Option<&SystemEvent> {
        self.parsed_events().iter().find_map(|e| {
            if let CliEvent::System(sys) = e {
                if sys.subtype == "init" {
                    return Some(sys);
                }
            }
            None
        })
    }

    /// Get first usage info.
    fn first_usage(&self) -> Option<&UsageInfo> {
        self.parsed_events().iter().find_map(|e| {
            if let CliEvent::Assistant(a) = e {
                a.message.usage.as_ref()
            } else {
                None
            }
        })
    }

    // ========================================================================
    // Behavioral helpers — event classification & sequencing
    // ========================================================================

    /// Classify each parsed event into an EventKind for sequence assertions.
    fn event_kinds(&self) -> Vec<EventKind> {
        self.parsed_events()
            .iter()
            .map(|e| match e {
                CliEvent::System(_) => EventKind::System,
                CliEvent::Assistant(_) => EventKind::Assistant,
                CliEvent::User(_) => EventKind::User,
                CliEvent::Result(_) => EventKind::Result,
                CliEvent::ControlRequest(req) => {
                    if let Some(ref input) = req.request.input {
                        match input.hook_event_name.as_str() {
                            "PreToolUse" => EventKind::PreToolUse,
                            "PostToolUse" => EventKind::PostToolUse,
                            _ => EventKind::ControlRequest,
                        }
                    } else {
                        EventKind::ControlRequest
                    }
                }
                CliEvent::ControlResponse(_) => EventKind::ControlResponse,
            })
            .collect()
    }

    /// Assert that `expected` appears as a subsequence (in order, not necessarily
    /// contiguous) within `event_kinds()`.
    fn assert_subsequence(&self, expected: &[EventKind]) {
        let kinds = self.event_kinds();
        let mut ei = 0; // index into expected
        for (i, kind) in kinds.iter().enumerate() {
            if ei < expected.len() && *kind == expected[ei] {
                ei += 1;
            }
            if ei == expected.len() {
                return; // all matched
            }
            let _ = i;
        }
        // Build a nice diff
        let kinds_str: Vec<_> = kinds.iter().map(|k| format!("{:?}", k)).collect();
        let expected_str: Vec<_> = expected.iter().map(|k| format!("{:?}", k)).collect();
        panic!(
            "Subsequence assertion failed.\n  Expected subsequence: {:?}\n  Matched {}/{} before failing.\n  Actual sequence: {:?}",
            expected_str, ei, expected.len(), kinds_str,
        );
    }

    // ========================================================================
    // Behavioral helpers — content extractors
    // ========================================================================

    /// Concatenated text from all Text ContentBlocks across all assistant events.
    fn all_text(&self) -> String {
        let mut out = String::new();
        for e in self.parsed_events() {
            if let CliEvent::Assistant(a) = e {
                for block in &a.message.content {
                    if let ContentBlock::Text(t) = block {
                        out.push_str(&t.text);
                    }
                }
            }
        }
        out
    }

    /// Returns `Vec<&Value>` of inputs for tool_use blocks with the given name.
    fn tool_use_inputs(&self, tool_name: &str) -> Vec<&Value> {
        let mut inputs = Vec::new();
        for e in self.parsed_events() {
            if let CliEvent::Assistant(a) = e {
                for block in &a.message.content {
                    if let ContentBlock::ToolUse(tu) = block {
                        if tu.name == tool_name {
                            inputs.push(&tu.input);
                        }
                    }
                }
            }
        }
        inputs
    }

    /// Returns `Vec<(tool_use_id, tool_name)>` from assistant ToolUse blocks.
    fn tool_use_ids(&self) -> Vec<(String, String)> {
        let mut ids = Vec::new();
        for e in self.parsed_events() {
            if let CliEvent::Assistant(a) = e {
                for block in &a.message.content {
                    if let ContentBlock::ToolUse(tu) = block {
                        ids.push((tu.id.clone(), tu.name.clone()));
                    }
                }
            }
        }
        ids
    }

    /// Extract tool result content from User events by navigating
    /// `extra["message"]["content"]` — the same path the adapter uses.
    fn tool_result_items(&self) -> Vec<(String, Value, bool)> {
        let mut results = Vec::new();
        for e in self.parsed_events() {
            if let CliEvent::User(u) = e {
                if let Some(message) = u.extra.get("message") {
                    if let Some(content_array) = message.get("content").and_then(|c| c.as_array())
                    {
                        for item in content_array {
                            if item.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                                let tool_use_id = item
                                    .get("tool_use_id")
                                    .and_then(|id| id.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let content = item
                                    .get("content")
                                    .cloned()
                                    .unwrap_or(Value::Null);
                                let is_error = item
                                    .get("is_error")
                                    .and_then(|e| e.as_bool())
                                    .unwrap_or(false);
                                results.push((tool_use_id, content, is_error));
                            }
                        }
                    }
                }
            }
        }
        results
    }

    /// Returns `Vec<String>` of tool_use_id from PreToolUse ControlRequests.
    fn pretool_hook_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        for e in self.parsed_events() {
            if let CliEvent::ControlRequest(req) = e {
                if let Some(ref input) = req.request.input {
                    if input.hook_event_name == "PreToolUse" {
                        if let Some(ref id) = req.request.tool_use_id {
                            ids.push(id.clone());
                        }
                    }
                }
            }
        }
        ids
    }

    // ========================================================================
    // Behavioral helpers — consistency assertions
    // ========================================================================

    /// For each tool_use_id in Assistant(tool_use), verify the same ID appears
    /// in PreToolUse ControlRequest and in User event tool_result. No orphans.
    fn assert_tool_id_consistency(&self) {
        let assistant_ids: Vec<(String, String)> = self.tool_use_ids();
        let pretool_ids: Vec<String> = self.pretool_hook_ids();
        let result_ids: Vec<String> = self.tool_result_items().iter().map(|(id, _, _)| id.clone()).collect();

        for (tool_use_id, tool_name) in &assistant_ids {
            assert!(
                pretool_ids.contains(tool_use_id),
                "Tool '{}' (id={}) in Assistant but missing from PreToolUse hooks.\n  PreToolUse IDs: {:?}",
                tool_name, tool_use_id, pretool_ids,
            );
            assert!(
                result_ids.contains(tool_use_id),
                "Tool '{}' (id={}) in Assistant but missing from User tool_results.\n  User result IDs: {:?}",
                tool_name, tool_use_id, result_ids,
            );
        }
        // Check for orphaned PreToolUse IDs
        let assistant_id_set: Vec<&str> = assistant_ids.iter().map(|(id, _)| id.as_str()).collect();
        for id in &pretool_ids {
            assert!(
                assistant_id_set.contains(&id.as_str()),
                "PreToolUse hook has tool_use_id={} not found in any Assistant tool_use block",
                id,
            );
        }
    }

    /// Verify System(init).session_id == Result.session_id.
    fn assert_session_id_consistency(&self) {
        let init = self.system_init_event().expect("Missing System init event");
        let result = self.result_event().expect("Missing Result event");
        let init_sid = init.session_id.as_ref().expect("System init missing session_id");
        let result_sid = result.session_id.as_ref().expect("Result missing session_id");
        assert_eq!(
            init_sid, result_sid,
            "Session ID mismatch: init={}, result={}",
            init_sid, result_sid,
        );
    }

    /// Print a human-readable summary of all events.
    fn print_event_summary(&self) {
        eprintln!("\n  === Event Summary ({} events) ===", self.events.len());
        for (i, entry) in self.events.iter().enumerate() {
            match entry {
                ParsedLine::Ok(event) => {
                    let desc = match event {
                        CliEvent::System(s) => format!("System({})", s.subtype),
                        CliEvent::Assistant(a) => {
                            let blocks: Vec<String> = a
                                .message
                                .content
                                .iter()
                                .map(|b| match b {
                                    ContentBlock::Text(t) => {
                                        format!("text({}ch)", t.text.len())
                                    }
                                    ContentBlock::ToolUse(tu) => {
                                        format!("tool_use({})", tu.name)
                                    }
                                    ContentBlock::Thinking(_) => "thinking".to_string(),
                                    ContentBlock::InputJsonDelta(_) => "delta".to_string(),
                                    ContentBlock::ToolResult(tr) => {
                                        format!("tool_result({})", tr.tool_use_id)
                                    }
                                })
                                .collect();
                            format!("Assistant [{}]", blocks.join(", "))
                        }
                        CliEvent::User(_) => "User".to_string(),
                        CliEvent::Result(r) => {
                            format!("Result({})", r.subtype)
                        }
                        CliEvent::ControlRequest(req) => {
                            let hook = req
                                .request
                                .input
                                .as_ref()
                                .map(|i| i.hook_event_name.as_str())
                                .unwrap_or(&req.request.subtype);
                            format!("ControlRequest({})", hook)
                        }
                        CliEvent::ControlResponse(_) => "ControlResponse(ack)".to_string(),
                    };
                    eprintln!("  [{:3}] OK  {}", i, desc);
                }
                ParsedLine::Err { error, .. } => {
                    eprintln!("  [{:3}] ERR {}", i, error);
                }
            }
        }
        eprintln!("  === End Summary ===\n");
    }
}

// ============================================================================
// Tests — Bypass Mode (no hooks)
// ============================================================================

#[test]
#[ignore]
fn test_basic_text() {
    eprintln!("\n--- test_basic_text ---");
    let h = CliHarness::run(HarnessMode::Bypass, "Reply with exactly: PONG");
    h.print_event_summary();

    assert!(
        h.parse_failures().is_empty(),
        "Parse failures: {:?}",
        h.parse_failures()
    );
    assert!(h.has_system_init(), "Missing system init event");
    assert!(h.has_text_content(), "Missing text content");
    assert!(h.has_result_success(), "Missing result success");
    assert!(h.has_usage_info(), "Missing usage info");
}

#[test]
#[ignore]
fn test_tool_read() {
    eprintln!("\n--- test_tool_read ---");
    let h = CliHarness::run(
        HarnessMode::Bypass,
        "Read the file /etc/shells and tell me how many lines it has. Use the Read tool.",
    );
    h.print_event_summary();

    assert!(
        h.parse_failures().is_empty(),
        "Parse failures: {:?}",
        h.parse_failures()
    );
    assert!(h.has_tool_use("Read"), "Missing Read tool use. Tools seen: {:?}", h.tool_use_names());
    assert!(h.has_result_success());
}

#[test]
#[ignore]
fn test_tool_bash() {
    eprintln!("\n--- test_tool_bash ---");
    let h = CliHarness::run(
        HarnessMode::Bypass,
        "Run: echo hello_from_test. Use the Bash tool.",
    );
    h.print_event_summary();

    assert!(
        h.parse_failures().is_empty(),
        "Parse failures: {:?}",
        h.parse_failures()
    );
    assert!(h.has_tool_use("Bash"), "Missing Bash tool use. Tools seen: {:?}", h.tool_use_names());
    assert!(h.has_result_success());
}

#[test]
#[ignore]
fn test_tool_write() {
    eprintln!("\n--- test_tool_write ---");
    let h = CliHarness::run(
        HarnessMode::Bypass,
        "Write a file called test_output.txt containing 'hello world'. Use the Write tool.",
    );
    h.print_event_summary();

    assert!(
        h.parse_failures().is_empty(),
        "Parse failures: {:?}",
        h.parse_failures()
    );
    assert!(
        h.has_tool_use("Write"),
        "Missing Write tool use. Tools seen: {:?}",
        h.tool_use_names()
    );
    assert!(h.has_result_success());
}

#[test]
#[ignore]
fn test_tool_glob() {
    eprintln!("\n--- test_tool_glob ---");
    let h = CliHarness::run(
        HarnessMode::Bypass,
        "Use the Glob tool to find all *.txt files in /tmp. Just list them.",
    );
    h.print_event_summary();

    assert!(
        h.parse_failures().is_empty(),
        "Parse failures: {:?}",
        h.parse_failures()
    );
    assert!(h.has_tool_use("Glob"), "Missing Glob tool use. Tools seen: {:?}", h.tool_use_names());
    assert!(h.has_result_success());
}

#[test]
#[ignore]
fn test_tool_grep() {
    eprintln!("\n--- test_tool_grep ---");
    let h = CliHarness::run(
        HarnessMode::Bypass,
        "Use the Grep tool to search for 'bash' in /etc/shells.",
    );
    h.print_event_summary();

    assert!(
        h.parse_failures().is_empty(),
        "Parse failures: {:?}",
        h.parse_failures()
    );
    assert!(h.has_tool_use("Grep"), "Missing Grep tool use. Tools seen: {:?}", h.tool_use_names());
    assert!(h.has_result_success());
}

#[test]
#[ignore]
fn test_multiple_tools() {
    eprintln!("\n--- test_multiple_tools ---");
    let h = CliHarness::run(
        HarnessMode::Bypass,
        "Do two things: 1) Run 'echo multi_test' using the Bash tool, 2) Read the file /etc/shells using the Read tool. Do both.",
    );
    h.print_event_summary();

    assert!(
        h.parse_failures().is_empty(),
        "Parse failures: {:?}",
        h.parse_failures()
    );
    let tools = h.tool_use_names();
    assert!(
        tools.len() >= 2,
        "Expected at least 2 tool uses, got {}: {:?}",
        tools.len(),
        tools
    );
    assert!(h.has_result_success());
}

#[test]
#[ignore]
fn test_token_usage() {
    eprintln!("\n--- test_token_usage ---");
    let h = CliHarness::run(HarnessMode::Bypass, "Say hello.");
    h.print_event_summary();

    assert!(h.parse_failures().is_empty());

    let usage = h.first_usage().expect("Should have usage info");
    assert!(
        usage.input_tokens > 0,
        "input_tokens should be > 0, got {}",
        usage.input_tokens
    );
    assert!(
        usage.output_tokens > 0,
        "output_tokens should be > 0, got {}",
        usage.output_tokens
    );
}

#[test]
#[ignore]
fn test_result_structure() {
    eprintln!("\n--- test_result_structure ---");
    let h = CliHarness::run(HarnessMode::Bypass, "Say OK.");
    h.print_event_summary();

    assert!(h.parse_failures().is_empty());

    let result = h.result_event().expect("Should have Result event");
    assert_eq!(result.subtype, "success");
    assert!(
        result.session_id.is_some(),
        "Result should have session_id for resume"
    );
}

#[test]
#[ignore]
fn test_init_event_structure() {
    eprintln!("\n--- test_init_event_structure ---");
    let h = CliHarness::run(HarnessMode::Bypass, "Say OK.");
    h.print_event_summary();

    assert!(h.parse_failures().is_empty());

    let init = h.system_init_event().expect("Should have System init event");
    assert_eq!(init.subtype, "init");
    assert!(
        init.session_id.is_some(),
        "System init should have session_id"
    );
}

// ============================================================================
// Tests — Hooks Mode (with permission callbacks)
// ============================================================================

#[test]
#[ignore]
fn test_hooks_lifecycle() {
    eprintln!("\n--- test_hooks_lifecycle ---");
    let h = CliHarness::run(
        HarnessMode::Hooks,
        "Run: echo hooks_test. Use the Bash tool.",
    );
    h.print_event_summary();

    assert!(
        h.parse_failures().is_empty(),
        "Parse failures: {:?}",
        h.parse_failures()
    );
    assert!(
        h.has_hook_callback("PreToolUse"),
        "Missing PreToolUse hook callback"
    );
    assert!(
        h.has_hook_callback("PostToolUse"),
        "Missing PostToolUse hook callback"
    );
    assert!(h.has_result_success());
}

#[test]
#[ignore]
fn test_hooks_have_tool_use_id() {
    eprintln!("\n--- test_hooks_have_tool_use_id ---");
    let h = CliHarness::run(
        HarnessMode::Hooks,
        "Read the file /etc/shells. Use the Read tool.",
    );
    h.print_event_summary();

    assert!(h.parse_failures().is_empty());

    // Find a PreToolUse ControlRequest and check it has tool_use_id
    let has_id = h.parsed_events().iter().any(|e| {
        if let CliEvent::ControlRequest(req) = e {
            if let Some(ref input) = req.request.input {
                if input.hook_event_name == "PreToolUse" {
                    return req.request.tool_use_id.is_some();
                }
            }
        }
        false
    });
    assert!(has_id, "PreToolUse hook callback should have tool_use_id");
}

#[test]
#[ignore]
fn test_hooks_have_tool_input() {
    eprintln!("\n--- test_hooks_have_tool_input ---");
    let h = CliHarness::run(
        HarnessMode::Hooks,
        "Run: echo input_test. Use the Bash tool.",
    );
    h.print_event_summary();

    assert!(h.parse_failures().is_empty());

    // Find a PreToolUse ControlRequest for Bash and check it has tool_input
    let has_input = h.parsed_events().iter().any(|e| {
        if let CliEvent::ControlRequest(req) = e {
            if let Some(ref input) = req.request.input {
                if input.hook_event_name == "PreToolUse"
                    && input.tool_name.as_deref() == Some("Bash")
                {
                    return input.tool_input.is_some();
                }
            }
        }
        false
    });
    assert!(
        has_input,
        "PreToolUse hook callback for Bash should have tool_input with command"
    );
}

#[test]
#[ignore]
fn test_control_response_acks() {
    eprintln!("\n--- test_control_response_acks ---");
    let h = CliHarness::run(
        HarnessMode::Hooks,
        "Run: echo ack_test. Use the Bash tool.",
    );
    h.print_event_summary();

    assert!(h.parse_failures().is_empty());

    let has_ack = h
        .parsed_events()
        .iter()
        .any(|e| matches!(e, CliEvent::ControlResponse(_)));
    assert!(has_ack, "Should have at least one ControlResponse ack");
}

#[test]
#[ignore]
fn test_zero_parse_failures() {
    eprintln!("\n--- test_zero_parse_failures ---");
    // Use a complex prompt that triggers multiple tools to stress-test parsing
    let h = CliHarness::run(
        HarnessMode::Hooks,
        "Do all three: 1) Run 'echo parse_test' with Bash, 2) Read /etc/shells with Read, 3) Tell me what you found.",
    );
    h.print_event_summary();

    let failures = h.parse_failures();
    assert!(
        failures.is_empty(),
        "Expected zero parse failures, got {}:\n{}",
        failures.len(),
        failures
            .iter()
            .map(|(line, err)| format!("  error: {}\n  line: {}", err, &line[..line.len().min(200)]))
            .collect::<Vec<_>>()
            .join("\n")
    );
    // Result event must be present (success or error_during_execution are both valid —
    // the key assertion is that every line parsed correctly)
    assert!(
        h.result_event().is_some(),
        "Should have a Result event"
    );
}

// ============================================================================
// Tests — Behavioral Contract (event ordering, content, ID consistency)
// ============================================================================

#[test]
#[ignore]
fn test_event_ordering_single_tool() {
    eprintln!("\n--- test_event_ordering_single_tool ---");
    let h = CliHarness::run(
        HarnessMode::Hooks,
        "Run echo hello. Use Bash.",
    );
    h.print_event_summary();

    assert!(h.parse_failures().is_empty(), "Parse failures: {:?}", h.parse_failures());
    // In hooks mode: System → Assistant(tool_use) → PreToolUse → PostToolUse → User(tool_result) → Result
    h.assert_subsequence(&[
        EventKind::System,
        EventKind::Assistant,
        EventKind::PreToolUse,
        EventKind::PostToolUse,
        EventKind::User,
        EventKind::Result,
    ]);
}

#[test]
#[ignore]
fn test_event_ordering_multi_tool() {
    eprintln!("\n--- test_event_ordering_multi_tool ---");
    let h = CliHarness::run(
        HarnessMode::Hooks,
        "Do two things in order: 1) Run 'echo first_tool' with Bash, 2) Read /etc/shells with Read.",
    );
    h.print_event_summary();

    assert!(h.parse_failures().is_empty(), "Parse failures: {:?}", h.parse_failures());

    // Each tool lifecycle should complete before the next starts.
    // At minimum: first tool's User (result) should come before second tool's PreToolUse.
    let kinds = h.event_kinds();

    // Find positions of User events and PreToolUse events
    let user_positions: Vec<usize> = kinds.iter().enumerate()
        .filter(|(_, k)| **k == EventKind::User)
        .map(|(i, _)| i)
        .collect();
    let pretool_positions: Vec<usize> = kinds.iter().enumerate()
        .filter(|(_, k)| **k == EventKind::PreToolUse)
        .map(|(i, _)| i)
        .collect();

    assert!(
        pretool_positions.len() >= 2,
        "Expected at least 2 PreToolUse events, got {}. Kinds: {:?}",
        pretool_positions.len(), kinds,
    );
    assert!(
        user_positions.len() >= 1,
        "Expected at least 1 User event, got {}. Kinds: {:?}",
        user_positions.len(), kinds,
    );

    // First User event (first tool's result) should come before second PreToolUse
    assert!(
        user_positions[0] < pretool_positions[1],
        "First tool's User result (pos={}) should come before second PreToolUse (pos={}). Kinds: {:?}",
        user_positions[0], pretool_positions[1], kinds,
    );
}

#[test]
#[ignore]
fn test_text_before_result() {
    eprintln!("\n--- test_text_before_result ---");
    let h = CliHarness::run(HarnessMode::Hooks, "Say hello");
    h.print_event_summary();

    assert!(h.parse_failures().is_empty());

    // The last meaningful event before Result should be an Assistant with text
    let events = h.parsed_events();
    let result_idx = events.iter().position(|e| matches!(e, CliEvent::Result(_)));
    assert!(result_idx.is_some(), "Missing Result event");

    let result_idx = result_idx.unwrap();
    // Look backwards from Result to find the last Assistant event
    let last_assistant_before = events[..result_idx].iter().rev()
        .find(|e| matches!(e, CliEvent::Assistant(_)));
    assert!(
        last_assistant_before.is_some(),
        "No Assistant event found before Result",
    );

    // That assistant should have text content (not just tool_use)
    if let Some(CliEvent::Assistant(a)) = last_assistant_before {
        let has_text = a.message.content.iter().any(|b| matches!(b, ContentBlock::Text(_)));
        assert!(has_text, "Last Assistant before Result should contain text. Content: {:?}",
            a.message.content.iter().map(|b| match b {
                ContentBlock::Text(_) => "text",
                ContentBlock::ToolUse(_) => "tool_use",
                ContentBlock::Thinking(_) => "thinking",
                ContentBlock::InputJsonDelta(_) => "delta",
                ContentBlock::ToolResult(_) => "tool_result",
            }).collect::<Vec<_>>()
        );
    }
}

#[test]
#[ignore]
fn test_bash_output_content() {
    eprintln!("\n--- test_bash_output_content ---");
    let h = CliHarness::run(
        HarnessMode::Hooks,
        "Run: echo CANARY_12345. Use Bash.",
    );
    h.print_event_summary();

    assert!(h.parse_failures().is_empty());

    let results = h.tool_result_items();
    assert!(!results.is_empty(), "No tool_result items found in User events");

    // At least one tool result should contain our canary string
    let has_canary = results.iter().any(|(_, content, _)| {
        let s = content.to_string();
        s.contains("CANARY_12345")
    });
    assert!(
        has_canary,
        "No tool_result contains 'CANARY_12345'. Results: {:?}",
        results,
    );
}

#[test]
#[ignore]
fn test_read_file_content() {
    eprintln!("\n--- test_read_file_content ---");
    let h = CliHarness::run(
        HarnessMode::Hooks,
        "Read /etc/shells. Use Read.",
    );
    h.print_event_summary();

    assert!(h.parse_failures().is_empty());

    let results = h.tool_result_items();
    assert!(!results.is_empty(), "No tool_result items found in User events");

    let has_shell_content = results.iter().any(|(_, content, _)| {
        let s = content.to_string();
        s.contains("/bin/sh") || s.contains("/bin/bash") || s.contains("/bin/zsh")
    });
    assert!(
        has_shell_content,
        "No tool_result contains expected shell paths. Results: {:?}",
        results,
    );
}

#[test]
#[ignore]
fn test_tool_input_structure() {
    eprintln!("\n--- test_tool_input_structure ---");
    let h = CliHarness::run(
        HarnessMode::Hooks,
        "Run: echo structure_test. Use Bash.",
    );
    h.print_event_summary();

    assert!(h.parse_failures().is_empty());

    // Bash tool_use input should have a `command` field
    let bash_inputs = h.tool_use_inputs("Bash");
    assert!(!bash_inputs.is_empty(), "No Bash tool_use inputs found");
    for input in &bash_inputs {
        assert!(
            input.get("command").is_some(),
            "Bash tool_use input missing 'command' field: {:?}",
            input,
        );
    }

    // PreToolUse hook for Bash should have matching tool_input with command
    let has_hook_input = h.parsed_events().iter().any(|e| {
        if let CliEvent::ControlRequest(req) = e {
            if let Some(ref input) = req.request.input {
                if input.hook_event_name == "PreToolUse"
                    && input.tool_name.as_deref() == Some("Bash")
                {
                    if let Some(ref tool_input) = input.tool_input {
                        return tool_input.get("command").is_some();
                    }
                }
            }
        }
        false
    });
    assert!(has_hook_input, "PreToolUse hook for Bash should have tool_input.command");
}

#[test]
#[ignore]
fn test_tool_id_consistency() {
    eprintln!("\n--- test_tool_id_consistency ---");
    let h = CliHarness::run(
        HarnessMode::Hooks,
        "Read /etc/shells. Use Read.",
    );
    h.print_event_summary();

    assert!(h.parse_failures().is_empty());
    h.assert_tool_id_consistency();
}

#[test]
#[ignore]
fn test_session_id_consistency() {
    eprintln!("\n--- test_session_id_consistency ---");
    let h = CliHarness::run(HarnessMode::Hooks, "Say OK");
    h.print_event_summary();

    assert!(h.parse_failures().is_empty());
    h.assert_session_id_consistency();
}

#[test]
#[ignore]
fn test_model_field_present() {
    eprintln!("\n--- test_model_field_present ---");
    let h = CliHarness::run(HarnessMode::Hooks, "Say OK");
    h.print_event_summary();

    assert!(h.parse_failures().is_empty());

    let has_model = h.parsed_events().iter().any(|e| {
        if let CliEvent::Assistant(a) = e {
            a.message.model.as_ref().map(|m| m.contains("haiku")).unwrap_or(false)
        } else {
            false
        }
    });
    assert!(
        has_model,
        "At least one AssistantMessage should have model field containing 'haiku'",
    );
}

#[test]
#[ignore]
fn test_tool_error_handling() {
    eprintln!("\n--- test_tool_error_handling ---");
    let h = CliHarness::run(
        HarnessMode::Hooks,
        "Read /tmp/nonexistent_file_xyz_12345. Use Read.",
    );
    h.print_event_summary();

    assert!(h.parse_failures().is_empty());

    let results = h.tool_result_items();
    assert!(!results.is_empty(), "No tool_result items found in User events");

    let has_error = results.iter().any(|(_, _, is_error)| *is_error);
    assert!(
        has_error,
        "Expected at least one tool_result with is_error=true. Results: {:?}",
        results,
    );
}
