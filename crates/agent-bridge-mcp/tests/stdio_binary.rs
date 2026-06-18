use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};
use uuid::Uuid;

static PROVIDER_READINESS_TEST_LOCK: Mutex<()> = Mutex::new(());

struct McpClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
    notifications: Vec<Value>,
}

struct FixtureEnv {
    _guard: MutexGuard<'static, ()>,
    root: PathBuf,
    state_dir: PathBuf,
    fake_provider: PathBuf,
    log_dir: PathBuf,
}

impl McpClient {
    fn start(env: &FixtureEnv) -> Self {
        let workspaces = std::env::join_paths([env.root.as_os_str()]).unwrap();
        Self::start_with_options(
            env,
            Some(workspaces),
            None,
            Some(&env.state_dir),
            BTreeMap::new(),
        )
    }

    fn start_with_workspace_value(env: &FixtureEnv, workspaces: OsString) -> Self {
        Self::start_with_options(
            env,
            Some(workspaces),
            None,
            Some(&env.state_dir),
            BTreeMap::new(),
        )
    }

    fn start_with_legacy_allowed_root_only(env: &FixtureEnv) -> Self {
        Self::start_with_options(
            env,
            None,
            Some(env.root.clone()),
            Some(&env.state_dir),
            BTreeMap::new(),
        )
    }

    fn start_without_workspace(env: &FixtureEnv) -> Self {
        Self::start_with_options(env, None, None, Some(&env.state_dir), BTreeMap::new())
    }

    fn start_without_state_dir(env: &FixtureEnv) -> Self {
        let workspaces = std::env::join_paths([env.root.as_os_str()]).unwrap();
        Self::start_with_options(env, Some(workspaces), None, None, BTreeMap::new())
    }

    fn start_with_extra_env(env: &FixtureEnv, extra_env: BTreeMap<String, OsString>) -> Self {
        let workspaces = std::env::join_paths([env.root.as_os_str()]).unwrap();
        Self::start_with_options(env, Some(workspaces), None, Some(&env.state_dir), extra_env)
    }

    fn start_with_options(
        env: &FixtureEnv,
        workspaces: Option<OsString>,
        legacy_allowed_root: Option<PathBuf>,
        state_dir: Option<&Path>,
        extra_env: BTreeMap<String, OsString>,
    ) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_agent-bridge-mcp"));
        command
            .env_remove("AGENT_BRIDGE_ALLOWED_ROOT")
            .env_remove("AGENT_BRIDGE_WORKSPACES")
            .env_remove("AGENT_BRIDGE_STATE_DIR")
            .env("HOME", &env.root)
            .env("CURSOR_AGENT_BIN", &env.fake_provider)
            .env("CURSOR_ACP_BIN", &env.fake_provider)
            .env("PI_BIN", &env.fake_provider)
            .env("KIMI_ACP_BIN", &env.fake_provider)
            .env("CODEX_BIN", &env.fake_provider)
            .env("CODEX_ACP_BIN", &env.fake_provider)
            .env("FORGE_BIN", &env.fake_provider)
            .env("FORGE_ACP_BIN", &env.fake_provider)
            .env("AGY_BIN", &env.fake_provider)
            .env("ANTIGRAVITY_ACP_BIN", &env.fake_provider)
            .env("ANTHROPIC_API_KEY", "test-key")
            .env("ANTHROPIC_AUTH_TOKEN", "test-auth-token")
            .env("CLAUDE_CODE_OAUTH_TOKEN", "test-code-oauth-token")
            .env("ANTHROPIC_BASE_URL", "http://127.0.0.1:8787")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(workspaces) = workspaces {
            command.env("AGENT_BRIDGE_WORKSPACES", workspaces);
        }
        if let Some(legacy_allowed_root) = legacy_allowed_root {
            command.env("AGENT_BRIDGE_ALLOWED_ROOT", legacy_allowed_root);
        }
        if let Some(state_dir) = state_dir {
            command.env("AGENT_BRIDGE_STATE_DIR", state_dir);
        }
        command.env("CLAUDE_BIN", &env.fake_provider);
        command.env("CLAUDE_ACP_BIN", &env.fake_provider);
        for (key, value) in extra_env {
            command.env(key, value);
        }
        let mut child = command.spawn().unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            stdout,
            next_id: 1,
            notifications: Vec::new(),
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        writeln!(self.stdin, "{request}").unwrap();
        self.stdin.flush().unwrap();

        loop {
            let message = self.read_message();
            if message.get("id").and_then(Value::as_i64) == Some(id) {
                return message;
            }
            if message.get("id").is_none() && message.get("method").is_some() {
                self.notifications.push(message);
                continue;
            }
            panic!("expected MCP response for id={id}, got {message}");
        }
    }

    fn read_message(&mut self) -> Value {
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        assert!(!line.is_empty(), "expected MCP message");
        serde_json::from_str(&line).unwrap()
    }

    fn notification(&mut self, method: &str) -> Value {
        if let Some(index) = self
            .notifications
            .iter()
            .position(|notification| notification["method"] == method)
        {
            return self.notifications.remove(index);
        }

        loop {
            let message = self.read_message();
            if message.get("id").is_none() && message.get("method").is_some() {
                if message["method"] == method {
                    return message;
                }
                self.notifications.push(message);
                continue;
            }
            panic!("expected MCP notification {method}, got {message}");
        }
    }

    fn initialize(&mut self, params: Value) -> Value {
        self.request("initialize", params)
    }

    fn tool(&mut self, name: &str, arguments: Value) -> Value {
        let response = self.request(
            "tools/call",
            json!({
                "name": name,
                "arguments": arguments
            }),
        );
        assert_ne!(response["result"]["isError"], true, "{response}");
        serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
    }

    fn raw_tool_response(&mut self, name: &str, arguments: Value) -> Value {
        let response = self.request(
            "tools/call",
            json!({
                "name": name,
                "arguments": arguments
            }),
        );
        assert_ne!(response["result"]["isError"], true, "{response}");
        response["result"].clone()
    }

    fn tool_with_meta(&mut self, name: &str, arguments: Value, meta: Value) -> Value {
        let response = self.request(
            "tools/call",
            json!({
                "name": name,
                "arguments": arguments,
                "_meta": meta
            }),
        );
        assert_ne!(response["result"]["isError"], true, "{response}");
        serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
    }

    fn tool_response_with_params(&mut self, params: Value) -> Value {
        self.request("tools/call", params)
    }

    fn tool_error(&mut self, name: &str, arguments: Value) -> String {
        let response = self.request(
            "tools/call",
            json!({
                "name": name,
                "arguments": arguments
            }),
        );
        assert_eq!(response["result"]["isError"], true, "{response}");
        response["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string()
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.stdin.flush();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn temp_dir(prefix: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("{prefix}-{}", Uuid::new_v4().simple()));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn fixture_env() -> FixtureEnv {
    let guard = provider_readiness_test_guard();
    let root = temp_dir("agent-bridge-root");
    let state_dir = temp_dir("agent-bridge-state");
    let log_dir = state_dir.join("provider-log");
    std::fs::create_dir_all(&log_dir).unwrap();
    let fake_provider = root.join("fake-provider");
    std::fs::write(
        &fake_provider,
        [
            "#!/bin/sh",
            "if [ -n \"$AGENT_BRIDGE_STATE_DIR\" ]; then",
            "  mkdir -p \"$AGENT_BRIDGE_STATE_DIR/provider-log\"",
            "  printf '%s\\n' \"$*\" > \"$AGENT_BRIDGE_STATE_DIR/provider-log/argv.txt\"",
            "fi",
            "case \"$*\" in",
            "  *--version*)",
            "  echo fake-provider 1.0.0",
            "  exit 0",
            "    ;;",
            "esac",
            "read init || exit 1",
            "printf '%s\\n' \"$init\" > \"$AGENT_BRIDGE_STATE_DIR/provider-log/stdin.txt\"",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{}}}'",
            "read new_session || exit 1",
            "printf '%s\\n' \"$new_session\" >> \"$AGENT_BRIDGE_STATE_DIR/provider-log/stdin.txt\"",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"sessionId\":\"test-session\"}}'",
            "read prompt_request || exit 1",
            "printf '%s\\n' \"$prompt_request\" >> \"$AGENT_BRIDGE_STATE_DIR/provider-log/stdin.txt\"",
            "text='fixture ok'",
            "case \"$prompt_request\" in",
            "  *echo-api-key-fail*)",
            "    echo \"$ANTHROPIC_API_KEY\" >&2",
            "    echo 'not-json-from-provider'",
            "    exit 0",
            "    ;;",
            "  *echo-api-key*) text=\"$ANTHROPIC_API_KEY\" ;;",
            "  *claude-timeout*)",
            "    printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"test-session\",\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"claude-task-started\"}}}}'",
            "    sleep 2 &",
            "    child=$!",
            "    trap 'kill -TERM \"$child\" 2>/dev/null || true; wait \"$child\" 2>/dev/null || true; exit 143' TERM INT",
            "    wait \"$child\"",
            "    ;;",
            "  *non-zero-exit*)",
            "    echo 'provider refused task' >&2",
            "    exit 42",
            "    ;;",
            "  *missing-result*) text='' ;;",
            "  *terminal-noise*) text='terminal probe noisefixture oktrailing noise' ;;",
            "  *malformed-output*)",
            "    echo 'terminal noise' >&2",
            "    echo 'not-json-from-provider'",
            "    exit 0",
            "    ;;",
            "  *secret-token-for-redaction*) text='secret-token-for-redaction' ;;",
            "  *.agent-bridge-unblocked-smoke*)",
            "    printf 'agent-bridge-smoke' > .agent-bridge-unblocked-smoke || exit 1",
            "    test \"$(cat .agent-bridge-unblocked-smoke)\" = 'agent-bridge-smoke' || exit 1",
            "    rm -f .agent-bridge-unblocked-smoke",
            "    text='AGENT_BRIDGE_PROVIDER_SMOKE_OK'",
            "    ;;",
            "  *AGENT_BRIDGE_PROVIDER_SMOKE_OK*) text='AGENT_BRIDGE_PROVIDER_SMOKE_OK' ;;",
            "  *malformed-json*)",
            "    printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"test-session\",\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"{\\\"type\\\":\"}}}}'",
            "    printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"stopReason\":\"end_turn\"}}'",
            "    exit 0",
            "    ;;",
            "  *final-then-timeout*)",
            "    printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"test-session\",\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"finished before timeout\"}}}}'",
            "    sleep 2",
            "    printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"stopReason\":\"end_turn\"}}'",
            "    exit 0",
            "    ;;",
            "  *sleep-long*)",
            "    printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"test-session\",\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"started-long\"}}}}'",
            "    echo waiting-long >&2",
            "    sleep 2 &",
            "    child=$!",
            "    trap 'kill -TERM \"$child\" 2>/dev/null || true; wait \"$child\" 2>/dev/null || true; exit 143' TERM INT",
            "    wait \"$child\"",
            "    ;;",
            "  *emit-logs*)",
            "    text='lifecycle-stdout'",
            "    echo lifecycle-stderr >&2",
            "    ;;",
            "esac",
            "if [ -n \"$text\" ]; then",
            "  printf '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"test-session\",\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"%s\"}}}}\\n' \"$text\"",
            "fi",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"stopReason\":\"end_turn\"}}'",
            "exit 0",
            "",
        ]
        .join("\n"),
    )
    .unwrap();
    make_executable(&fake_provider);
    FixtureEnv {
        _guard: guard,
        root,
        state_dir,
        fake_provider,
        log_dir,
    }
}

fn write_fake_provider(env: &FixtureEnv, lines: &[&str]) {
    std::fs::write(&env.fake_provider, lines.join("\n")).unwrap();
    make_executable(&env.fake_provider);
}

fn provider_keys(value: &Value) -> Vec<String> {
    value["providers"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect()
}

fn provider_readiness_test_guard() -> MutexGuard<'static, ()> {
    PROVIDER_READINESS_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn sorted_provider_keys(value: &Value) -> Vec<String> {
    let mut keys = provider_keys(value);
    keys.sort();
    keys
}

fn acp_router_command(env: &FixtureEnv) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_agent-bridge-mcp"));
    command
        .arg("acp-router")
        .env_remove("AGENT_BRIDGE_ALLOWED_ROOT")
        .env("HOME", &env.root)
        .env("AGENT_BRIDGE_WORKSPACES", &env.root)
        .env("AGENT_BRIDGE_STATE_DIR", &env.state_dir)
        .env("CLAUDE_BIN", &env.fake_provider)
        .env("CLAUDE_ACP_BIN", &env.fake_provider)
        .env("CODEX_BIN", &env.fake_provider)
        .env("CODEX_ACP_BIN", &env.fake_provider);
    command
}

fn json_lines(bytes: &[u8]) -> Vec<Value> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn read_json_line(stdout: &mut BufReader<ChildStdout>) -> Value {
    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    assert!(!line.is_empty(), "expected JSON-RPC line");
    serde_json::from_str(&line).unwrap()
}

fn read_json_response(stdout: &mut BufReader<ChildStdout>, id: i64) -> (Vec<Value>, Value) {
    let mut notifications = Vec::new();
    loop {
        let message = read_json_line(stdout);
        if message.get("id").and_then(Value::as_i64) == Some(id) {
            return (notifications, message);
        }
        notifications.push(message);
    }
}

fn start_acp_router_session(
    env: &FixtureEnv,
) -> (Child, ChildStdin, BufReader<ChildStdout>, String) {
    let mut child = acp_router_command(env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    writeln!(
        stdin,
        "{}",
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{}}})
    )
    .unwrap();
    assert_eq!(read_json_line(&mut stdout)["result"]["protocolVersion"], 1);
    writeln!(
        stdin,
        "{}",
        json!({"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd": env.root}})
    )
    .unwrap();
    let session = read_json_line(&mut stdout);
    let session_id = session["result"]["sessionId"].as_str().unwrap().to_string();
    (child, stdin, stdout, session_id)
}

fn write_acp_router_claude_review_prompt(
    stdin: &mut ChildStdin,
    session_id: &str,
    prompt_text: &str,
) {
    writeln!(
        stdin,
        "{}",
        json!({
            "jsonrpc":"2.0",
            "id":3,
            "method":"session/prompt",
            "params":{
                "sessionId": session_id,
                "prompt":[{"type":"text","text":prompt_text}],
                "policy":{"candidates":["claude"]},
                "mode":"review",
                "timeoutSeconds":5
            }
        })
    )
    .unwrap();
}

fn review_actions_text(result: &Value) -> String {
    serde_json::to_string(&result["reviewPacket"]["recommendedActions"]).unwrap()
}

fn assert_no_task_id_key(value: &Value) {
    match value {
        Value::Object(object) => {
            assert!(
                !object.contains_key("taskId"),
                "public payload should not contain taskId: {value}"
            );
            for child in object.values() {
                assert_no_task_id_key(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                assert_no_task_id_key(child);
            }
        }
        _ => {}
    }
}

fn make_executable(path: &Path) {
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).unwrap();
}

fn init_git_repo(root: &Path) {
    run_git(root, &["init"]);
    run_git(root, &["config", "user.email", "agent-bridge@example.test"]);
    run_git(root, &["config", "user.name", "Agent Bridge"]);
    run_git(root, &["config", "commit.gpgsign", "false"]);
    std::fs::write(root.join("README.md"), "fixture\n").unwrap();
    run_git(root, &["add", "README.md"]);
    run_git(root, &["commit", "-m", "fixture"]);
}

fn run_git(cwd: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
fn process_is_alive(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}

#[cfg(not(unix))]
fn process_is_alive(_pid: i32) -> bool {
    false
}

#[test]
fn stdio_protocol_and_tool_schema_smoke() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let initialize = client.request("initialize", json!({}));
    assert_eq!(initialize["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(
        initialize["result"]["capabilities"],
        json!({ "tools": {}, "prompts": {}, "resources": {} })
    );
    assert_eq!(
        initialize["result"]["serverInfo"]["name"],
        "agent-bridge-mcp"
    );
    let instructions = initialize["result"]["instructions"].as_str().unwrap();
    assert!(instructions.contains("prefer a provider different from the calling agent"));
    assert!(instructions.contains("Provider output is evidence only"));
    assert!(
        instructions[..512.min(instructions.len())]
            .contains("caller still owns project verification")
    );

    let tools = client.request("tools/list", json!({}));
    let tools = tools["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 8);
    let names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        vec![
            "providers_list",
            "doctor",
            "agent_spawn",
            "agent_list",
            "agent_observe",
            "agent_result",
            "agent_stop",
            "agent_remove"
        ]
    );
    for removed in [
        "providers_check",
        "agent_preview",
        "agent_status",
        "agent_wait",
        "agent_logs",
        "agent_transcript",
    ] {
        assert!(!names.contains(&removed), "removed tool listed: {removed}");
    }
    assert!(
        names.iter().all(|name| !name.starts_with("task_")),
        "legacy task_* lifecycle tools should not be listed: {names:?}"
    );
    let find = |name: &str| tools.iter().find(|tool| tool["name"] == name).unwrap();
    let agent_spawn = find("agent_spawn");
    let agent_list = find("agent_list");
    let agent_observe = find("agent_observe");
    let agent_result = find("agent_result");
    let doctor = find("doctor");
    assert_eq!(doctor["inputSchema"]["additionalProperties"], json!(false));
    assert_eq!(doctor["inputSchema"]["required"], json!([]));
    assert_eq!(
        doctor["inputSchema"]["properties"]["focus"]["enum"],
        json!(["all", "providers"])
    );
    assert_eq!(
        doctor["inputSchema"]["properties"]["providers"]["items"]["enum"],
        json!(["claude", "cursor", "kimi", "codex", "forge", "antigravity"])
    );
    assert_eq!(
        doctor["inputSchema"]["properties"]["aggregateTimeoutMs"]["maximum"],
        120000
    );
    assert_eq!(
        doctor["inputSchema"]["properties"]["providerTimeoutMs"]["additionalProperties"]["maximum"],
        90000
    );
    assert_eq!(
        doctor["inputSchema"]["properties"]["profile"]["enum"],
        json!(["bridge", "bare", "unblocked"])
    );
    assert_eq!(
        doctor["outputSchema"]["properties"]["launchReadiness"]["type"],
        "object"
    );
    assert_eq!(
        agent_spawn["inputSchema"]["properties"]["provider"]["enum"],
        json!(["claude", "cursor", "kimi", "codex", "forge", "antigravity"])
    );
    assert_eq!(
        agent_spawn["inputSchema"]["properties"]["profile"]["enum"],
        json!(["bridge", "bare", "unblocked"])
    );
    assert_eq!(
        agent_spawn["inputSchema"]["required"],
        json!(["provider", "mode", "prompt"])
    );
    assert_eq!(
        agent_spawn["inputSchema"]["additionalProperties"],
        json!(false)
    );
    assert!(
        agent_spawn["inputSchema"]["properties"]
            .get("dryRun")
            .is_some()
    );
    assert!(
        agent_spawn["description"]
            .as_str()
            .unwrap()
            .contains("Primary follow-ups")
    );
    assert_eq!(
        agent_list["inputSchema"]["additionalProperties"],
        json!(false)
    );
    assert!(
        agent_list["inputSchema"]["properties"]
            .get("scope")
            .is_none()
    );
    assert!(
        agent_list["inputSchema"]["properties"]
            .get("presentation")
            .is_none()
    );
    assert_eq!(
        agent_list["inputSchema"]["properties"]["limit"]["maximum"],
        100
    );
    assert_eq!(
        agent_list["outputSchema"]["properties"]["agents"]["type"],
        "array"
    );
    assert!(
        agent_observe["inputSchema"]["properties"]
            .get("until")
            .is_some()
    );
    assert_eq!(
        agent_observe["inputSchema"]["properties"]["timeoutMs"]["maximum"],
        120000
    );
    assert_eq!(
        agent_observe["outputSchema"]["properties"]["progress"]["type"],
        "object"
    );
    assert!(
        agent_result["inputSchema"]["properties"]
            .get("sections")
            .is_some()
    );
    for tool_name in ["agent_observe", "agent_result"] {
        let tool = find(tool_name);
        assert_eq!(tool["outputSchema"]["properties"]["next"]["type"], "array");
    }

    let providers_response = client.raw_tool_response("providers_list", json!({}));
    let providers_from_text: Value =
        serde_json::from_str(providers_response["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(providers_response["structuredContent"], providers_from_text);

    let prompts = client.request("prompts/list", json!({}));
    let prompts = prompts["result"]["prompts"].as_array().unwrap();
    assert_eq!(prompts.len(), 7);
    assert!(
        prompts
            .iter()
            .any(|prompt| prompt["name"] == "agent_bridge_delegate_implementation")
    );
    assert!(
        prompts
            .iter()
            .any(|prompt| prompt["name"] == "agent_bridge_claude_host_lifecycle")
    );
    assert!(
        prompts
            .iter()
            .any(|prompt| prompt["name"] == "agent_bridge_dogfood_workflows")
    );
    assert!(
        prompts
            .iter()
            .any(|prompt| prompt["name"] == "agent_bridge_compare_providers")
    );

    let prompt = client.request(
        "prompts/get",
        json!({"name": "agent_bridge_delegate_implementation"}),
    );
    let prompt_text = prompt["result"]["messages"][0]["content"]["text"]
        .as_str()
        .unwrap();
    assert!(prompt_text.contains("agent_spawn"));
    assert!(prompt_text.contains("agent_observe"));
    assert!(prompt_text.contains("dryRun"));
    assert!(prompt_text.contains("main caller remains responsible"));

    let resources = client.request("resources/list", json!({}));
    let resources = resources["result"]["resources"].as_array().unwrap();
    assert_eq!(resources.len(), 6);
    assert!(
        resources
            .iter()
            .any(|resource| resource["uri"] == "agent-bridge://guidance/caller-workflow")
    );
    assert!(
        resources
            .iter()
            .any(|resource| resource["uri"] == "agent-bridge://guidance/claude-host-lifecycle")
    );
    assert!(
        resources
            .iter()
            .any(|resource| resource["uri"] == "agent-bridge://guidance/dogfood-workflows")
    );

    let resource = client.request(
        "resources/read",
        json!({"uri": "agent-bridge://guidance/caller-workflow"}),
    );
    let resource_content = &resource["result"]["contents"][0];
    assert_eq!(resource_content["mimeType"], "text/markdown");
    assert!(
        resource_content["text"]
            .as_str()
            .unwrap()
            .contains("agent_result")
    );
    assert!(
        resource_content["text"]
            .as_str()
            .unwrap()
            .contains("reviewPacket")
    );

    let host_lifecycle = client.request(
        "resources/read",
        json!({"uri": "agent-bridge://guidance/claude-host-lifecycle"}),
    );
    let host_lifecycle_text = host_lifecycle["result"]["contents"][0]["text"]
        .as_str()
        .unwrap();
    assert!(host_lifecycle_text.contains("claude-host-runner"));
    assert!(host_lifecycle_text.contains("ping"));
    assert!(host_lifecycle_text.contains("workspace_policy_mismatch"));

    let dogfood = client.request(
        "resources/read",
        json!({"uri": "agent-bridge://guidance/dogfood-workflows"}),
    );
    let dogfood_text = dogfood["result"]["contents"][0]["text"].as_str().unwrap();
    assert!(dogfood_text.contains("read-only review"));
    assert!(dogfood_text.contains("isolated implementation"));
    assert!(dogfood_text.contains("provider comparison"));

    let missing_resource = client.request("resources/read", json!({"uri": "file:///etc/passwd"}));
    assert_eq!(missing_resource["error"]["code"], -32002);

    let missing = client.request("missing/method", json!({}));
    assert_eq!(missing["error"]["code"], -32601);
}

#[test]
fn stdio_binary_prints_help_and_version_without_starting_mcp_loop() {
    let help = Command::new(env!("CARGO_BIN_EXE_agent-bridge-mcp"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(help.status.success());
    let help_stdout = String::from_utf8(help.stdout).unwrap();
    assert!(help_stdout.contains("Usage:"));
    assert!(help_stdout.contains("--config-check"));
    assert!(help_stdout.contains("--doctor-smoke"));
    assert!(help_stdout.contains("claude-host-runner"));

    let version = Command::new(env!("CARGO_BIN_EXE_agent-bridge-mcp"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(version.status.success());
    let version_stdout = String::from_utf8(version.stdout).unwrap();
    assert!(version_stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn stdio_binary_exposes_acp_router_runtime_without_starting_mcp_loop() {
    let help = Command::new(env!("CARGO_BIN_EXE_agent-bridge-mcp"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(help.status.success());
    let help_stdout = String::from_utf8(help.stdout).unwrap();
    assert!(help_stdout.contains("acp-router"));

    let env = fixture_env();
    let output = Command::new(env!("CARGO_BIN_EXE_agent-bridge-mcp"))
        .arg("acp-router")
        .env_remove("AGENT_BRIDGE_ALLOWED_ROOT")
        .env("AGENT_BRIDGE_WORKSPACES", &env.root)
        .env("AGENT_BRIDGE_STATE_DIR", &env.state_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr = {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(!env.state_dir.join("server.pid").exists());
}

#[test]
fn stdio_acp_router_initializes_and_creates_session_without_provider_launch() {
    let env = fixture_env();
    let mut child = acp_router_command(&env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    writeln!(
        stdin,
        "{}",
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{}}})
    )
    .unwrap();
    writeln!(
        stdin,
        "{}",
        json!({"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd": env.root}})
    )
    .unwrap();
    drop(stdin);

    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stderr = {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let messages = json_lines(&output.stdout);
    assert_eq!(messages.len(), 2, "{messages:?}");
    assert_eq!(messages[0]["id"], 1);
    assert_eq!(messages[0]["result"]["protocolVersion"], 1);
    assert!(messages[0]["result"]["agentCapabilities"].is_object());
    assert!(messages[0]["result"].get("tools").is_none());
    assert_eq!(messages[1]["id"], 2);
    assert!(messages[1]["result"]["sessionId"].as_str().is_some());
    assert!(!env.log_dir.join("argv.txt").exists());
}

#[test]
fn stdio_acp_router_prompt_runs_one_provider_turn() {
    let env = fixture_env();
    let (mut child, mut stdin, mut stdout, session_id) = start_acp_router_session(&env);
    assert!(!env.log_dir.join("argv.txt").exists());

    write_acp_router_claude_review_prompt(&mut stdin, &session_id, "router-turn");

    let evidence_update = read_json_line(&mut stdout);
    assert_eq!(evidence_update["method"], "session/update");
    assert_eq!(evidence_update["params"]["sessionId"], session_id);
    assert_eq!(
        evidence_update["params"]["update"]["sessionUpdate"],
        "agent_bridge_evidence"
    );
    assert_eq!(
        evidence_update["params"]["update"]["agentBridgeEvidence"]["provider"],
        "claude"
    );
    assert_eq!(
        evidence_update["params"]["update"]["agentBridgeEvidence"]["kind"],
        "provider_event"
    );
    assert!(
        evidence_update["params"]["update"]["agentBridgeEvidence"]["eventIndex"]
            .as_u64()
            .is_some()
    );
    assert_eq!(
        evidence_update["params"]["update"]["agentBridgeEvidence"]["bounded"]["limit"],
        20
    );
    assert_eq!(
        evidence_update["params"]["update"]["agentBridgeEvidence"]["bounded"]["truncated"],
        false
    );
    let evidence = &evidence_update["params"]["update"]["agentBridgeEvidence"];
    for raw_key in ["raw", "stdout", "stderr", "transcript", "gitDiff"] {
        assert!(evidence.get(raw_key).is_none(), "{raw_key}");
    }

    let final_update = read_json_line(&mut stdout);
    assert_eq!(final_update["method"], "session/update");
    assert_eq!(final_update["params"]["sessionId"], session_id);
    assert_eq!(
        final_update["params"]["update"]["sessionUpdate"],
        "agent_message_chunk"
    );
    assert_eq!(
        final_update["params"]["update"]["content"]["text"],
        "fixture ok"
    );

    let response = read_json_line(&mut stdout);
    assert_eq!(response["id"], 3);
    assert_eq!(response["result"]["stopReason"], "end_turn");
    assert_eq!(response["result"]["routerResult"]["provider"], "claude");
    assert_eq!(response["result"]["routerResult"]["terminalKind"], "answer");
    assert_eq!(
        response["result"]["routerResult"]["finalText"],
        "fixture ok"
    );
    assert!(
        response["result"]["routerResult"]["attempts"][0]["evidenceRef"]["agentId"]
            .as_str()
            .is_some()
    );
    let router_result = &response["result"]["routerResult"];
    for raw_key in ["stdout", "stderr", "transcript", "gitDiff"] {
        assert!(router_result.get(raw_key).is_none(), "{raw_key}");
    }
    assert!(env.log_dir.join("argv.txt").exists());

    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success());
}

#[test]
fn stdio_acp_router_prompt_returns_blocker_for_refusal_stop_reason() {
    let env = fixture_env();
    write_fake_provider(
        &env,
        &[
            "#!/bin/sh",
            "case \"$*\" in",
            "  *--version*) echo fake-provider 1.0.0; exit 0 ;;",
            "esac",
            "read init || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{}}}'",
            "read new_session || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"sessionId\":\"test-session\"}}'",
            "read prompt_request || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"test-session\",\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"cannot comply\"}}}}'",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"stopReason\":\"refusal\"}}'",
            "exit 0",
        ],
    );
    let (mut child, mut stdin, mut stdout, session_id) = start_acp_router_session(&env);

    write_acp_router_claude_review_prompt(&mut stdin, &session_id, "please refuse");

    let (updates, response) = read_json_response(&mut stdout, 3);
    assert!(
        updates
            .iter()
            .all(|update| update["method"] == "session/update"),
        "{updates:?}"
    );
    assert_eq!(response["result"]["stopReason"], "refusal");
    assert_eq!(response["result"]["routerResult"]["provider"], "claude");
    assert_eq!(
        response["result"]["routerResult"]["terminalKind"],
        "blocker"
    );
    assert_eq!(
        response["result"]["routerResult"]["blockerReason"],
        "refusal"
    );
    assert_eq!(
        response["result"]["routerResult"]["failureCategory"],
        "provider_output_error"
    );
    assert_eq!(
        response["result"]["routerResult"]["attempts"][0]["disposition"],
        "blocker"
    );
    assert!(response["result"]["routerResult"]["finalText"].is_null());

    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success());
}

#[test]
fn stdio_acp_router_prompt_returns_classified_failure_without_final_text() {
    let env = fixture_env();
    let (mut child, mut stdin, mut stdout, session_id) = start_acp_router_session(&env);

    write_acp_router_claude_review_prompt(&mut stdin, &session_id, "malformed-output");

    let (_updates, response) = read_json_response(&mut stdout, 3);
    assert_eq!(response["result"]["stopReason"], "end_turn");
    assert_eq!(response["result"]["routerResult"]["provider"], "claude");
    assert_eq!(
        response["result"]["routerResult"]["terminalKind"],
        "failure"
    );
    assert_eq!(
        response["result"]["routerResult"]["failureCategory"],
        "provider_output_error"
    );
    assert_eq!(
        response["result"]["routerResult"]["attempts"][0]["disposition"],
        "terminal_failure"
    );
    assert!(response["result"]["routerResult"]["blockerReason"].is_null());
    assert!(response["result"]["routerResult"]["finalText"].is_null());

    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success());
}

#[test]
fn stdio_binary_config_check_prints_effective_config_json() {
    let env = fixture_env();
    let config_dir = env.root.join(".agent-bridge-mcp");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.toml"),
        format!(
            "workspaces = [\"{}\"]\nstate_dir = \"{}\"\nmax_active_tasks = 7\n",
            env.root.display(),
            env.state_dir.display()
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_agent-bridge-mcp"))
        .arg("--config-check")
        .env("HOME", &env.root)
        .env_remove("AGENT_BRIDGE_WORKSPACES")
        .env_remove("AGENT_BRIDGE_STATE_DIR")
        .output()
        .unwrap();

    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["status"], "ok");
    assert_eq!(value["valid"], true);
    assert_eq!(value["maxActiveTasks"], 7);
    assert_eq!(value["stateDir"], env.state_dir.display().to_string());
    assert_eq!(
        value["workspaces"],
        json!([env.root.canonicalize().unwrap().display().to_string()])
    );
}

#[test]
fn stdio_binary_config_check_keeps_legacy_warnings_on_stderr() {
    let env = fixture_env();
    let output = Command::new(env!("CARGO_BIN_EXE_agent-bridge-mcp"))
        .arg("--config-check")
        .env("HOME", &env.root)
        .env("AGENT_BRIDGE_WORKSPACES", &env.root)
        .env("AGENT_BRIDGE_STATE_DIR", &env.state_dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(stdout["status"], "ok");

    let stderr = String::from_utf8(output.stderr).unwrap();
    let warning = stderr
        .lines()
        .find_map(|line| serde_json::from_str::<Value>(line).ok())
        .expect("expected JSON warning log on stderr");
    assert_eq!(warning["level"], "WARN");
    assert_eq!(warning["fields"]["env_var"], "AGENT_BRIDGE_WORKSPACES");
    assert!(
        warning["fields"]["message"]
            .as_str()
            .unwrap()
            .contains("legacy Agent Bridge environment variable is deprecated")
    );
}

#[test]
fn stdio_binary_panic_hook_logs_json_to_stderr_without_stdout() {
    let output = Command::new(env!("CARGO_BIN_EXE_agent-bridge-mcp"))
        .env("AGENT_BRIDGE_FORCE_PANIC", "1")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).unwrap();
    let log_line = stderr
        .lines()
        .find(|line| line.trim_start().starts_with('{'))
        .expect("expected at least one JSON log line on stderr");
    let log: Value = serde_json::from_str(log_line).unwrap();
    assert_eq!(log["level"], "ERROR");
    assert!(log["fields"]["message"].as_str().unwrap().contains("panic"));
}

#[test]
fn stdio_binary_doctor_smoke_prints_provider_report_json() {
    let env = fixture_env();
    let output = Command::new(env!("CARGO_BIN_EXE_agent-bridge-mcp"))
        .arg("--doctor-smoke")
        .arg("--provider")
        .arg("cursor")
        .arg("--provider")
        .arg("codex")
        .env("HOME", &env.root)
        .env("AGENT_BRIDGE_WORKSPACES", &env.root)
        .env("AGENT_BRIDGE_STATE_DIR", &env.state_dir)
        .env("CURSOR_AGENT_BIN", &env.fake_provider)
        .env("CURSOR_ACP_BIN", &env.fake_provider)
        .env("CODEX_BIN", &env.fake_provider)
        .env("CODEX_ACP_BIN", &env.fake_provider)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(provider_keys(&value), vec!["cursor", "codex"]);
    assert_eq!(value["launchReadiness"]["status"], "ready");
    assert_eq!(value["providers"]["codex"]["startupVerified"], true);
    assert_eq!(value["providers"]["cursor"]["startupVerified"], true);
}

#[test]
fn stdio_binary_reload_refreshes_workspace_roots_from_pid_file() {
    let env = fixture_env();
    let config_dir = env.root.join(".agent-bridge-mcp");
    std::fs::create_dir_all(&config_dir).unwrap();
    let config_path = config_dir.join("config.toml");
    std::fs::write(
        &config_path,
        format!(
            "workspaces = [\"{}\"]\nstate_dir = \"{}\"\n",
            env.root.display(),
            env.state_dir.display()
        ),
    )
    .unwrap();

    let added_root = temp_dir("agent-bridge-reload-root");
    let mut client = McpClient::start_without_workspace(&env);
    client.initialize(json!({}));

    assert!(env.state_dir.join("server.pid").exists());
    let mut second_client = McpClient::start_without_workspace(&env);
    let second_initialize = second_client.initialize(json!({}));
    assert_eq!(
        second_initialize["result"]["serverInfo"]["name"],
        "agent-bridge-mcp"
    );

    let rejected_before_update = client.tool_error(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "research",
            "prompt": "hello",
            "cwd": added_root,
            "timeoutSeconds": 5
        }),
    );
    assert!(rejected_before_update.contains("cwd is outside configured workspaces"));

    std::fs::write(
        &config_path,
        format!(
            "workspaces = [\"{}\"]\nstate_dir = \"{}\"\n",
            added_root.display(),
            env.state_dir.display()
        ),
    )
    .unwrap();
    let rejected_before_reload = client.tool_error(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "research",
            "prompt": "hello",
            "cwd": added_root,
            "timeoutSeconds": 5
        }),
    );
    assert!(rejected_before_reload.contains("cwd is outside configured workspaces"));

    let reload = Command::new(env!("CARGO_BIN_EXE_agent-bridge-mcp"))
        .arg("reload")
        .env("HOME", &env.root)
        .env("AGENT_BRIDGE_STATE_DIR", &env.state_dir)
        .env_remove("AGENT_BRIDGE_WORKSPACES")
        .output()
        .unwrap();
    assert!(
        reload.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&reload.stdout),
        String::from_utf8_lossy(&reload.stderr)
    );

    let mut accepted_after_reload = None;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        let response = client.tool_response_with_params(json!({
            "name": "agent_spawn",
            "arguments": {
                "provider": "codex",
                "mode": "research",
                "prompt": "hello",
                "cwd": added_root,
                "timeoutSeconds": 5
            }
        }));
        if response["result"]["isError"] != true {
            accepted_after_reload = Some(response);
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(
        accepted_after_reload.is_some(),
        "expected added workspace to be accepted after reload"
    );
    let rejected_removed_root = client.tool_error(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "research",
            "prompt": "hello",
            "cwd": env.root,
            "timeoutSeconds": 5
        }),
    );
    assert!(rejected_removed_root.contains("cwd is outside configured workspaces"));

    std::fs::write(&config_path, "{not-toml").unwrap();
    let broken_reload = Command::new(env!("CARGO_BIN_EXE_agent-bridge-mcp"))
        .arg("reload")
        .env("HOME", &env.root)
        .env("AGENT_BRIDGE_STATE_DIR", &env.state_dir)
        .env_remove("AGENT_BRIDGE_WORKSPACES")
        .output()
        .unwrap();
    assert!(broken_reload.status.success());
    std::thread::sleep(Duration::from_millis(100));
    let still_accepted = client.tool(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "research",
            "prompt": "hello",
            "cwd": added_root,
            "timeoutSeconds": 5
        }),
    );
    assert_eq!(
        still_accepted["cwd"],
        added_root.canonicalize().unwrap().display().to_string()
    );
}

#[test]
fn stdio_concurrent_clients_see_each_others_tasks() {
    let env = fixture_env();
    let mut first = McpClient::start(&env);
    let mut second = McpClient::start(&env);
    first.initialize(json!({}));
    second.initialize(json!({}));
    let initially_listed = second.tool("agent_list", json!({"limit": 10}));
    assert!(initially_listed["agents"].as_array().unwrap().is_empty());

    let spawned = first.tool(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "hello from first client",
            "cwd": env.root,
            "timeoutSeconds": 5
        }),
    );
    let agent_id = spawned["agentId"].as_str().unwrap();
    let completed = first.tool(
        "agent_observe",
        json!({"agentId": agent_id, "until": "final", "timeoutMs": 30000}),
    );
    assert_eq!(completed["status"], "succeeded");

    let listed = second.tool("agent_list", json!({"limit": 10}));
    let listed_ids = listed["agents"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|agent| agent["agentId"].as_str())
        .collect::<Vec<_>>();

    assert!(
        listed_ids.contains(&agent_id),
        "second client list should include task spawned by first client: {listed}"
    );
}

#[test]
fn stdio_concurrent_clients_wait_for_each_others_tasks() {
    let env = fixture_env();
    let mut first = McpClient::start(&env);
    let mut second = McpClient::start(&env);
    first.initialize(json!({}));
    second.initialize(json!({}));
    let initially_listed = second.tool("agent_list", json!({"limit": 10}));
    assert!(initially_listed["agents"].as_array().unwrap().is_empty());

    let spawned = first.tool(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "final-then-timeout",
            "cwd": env.root,
            "timeoutSeconds": 5
        }),
    );
    let agent_id = spawned["agentId"].as_str().unwrap();

    let started = Instant::now();
    let waited = second.tool(
        "agent_observe",
        json!({"agentId": agent_id, "until": "final", "timeoutMs": 30000}),
    );

    assert_eq!(waited["status"], "succeeded");
    assert_eq!(waited.get("timedOut"), None);
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "foreign-client wait should poll persisted state instead of sleeping until the full observe timeout"
    );
}

#[test]
fn stdio_concurrent_clients_can_stop_each_others_tasks() {
    let env = fixture_env();
    let mut first = McpClient::start(&env);
    let mut second = McpClient::start(&env);
    first.initialize(json!({}));
    second.initialize(json!({}));
    let initially_listed = second.tool("agent_list", json!({"limit": 10}));
    assert!(initially_listed["agents"].as_array().unwrap().is_empty());

    let spawned = first.tool(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "sleep-long",
            "cwd": env.root,
            "timeoutSeconds": 20
        }),
    );
    let agent_id = spawned["agentId"].as_str().unwrap();

    let stopped = second.tool("agent_stop", json!({"agentId": agent_id}));
    assert_eq!(stopped["status"], "stopped");

    let observed = first.tool(
        "agent_observe",
        json!({"agentId": agent_id, "until": "final", "timeoutMs": 30000}),
    );
    assert_eq!(observed["status"], "stopped");
    assert_eq!(observed["isFinal"], true);
}

#[test]
fn stdio_agent_extension_readiness_reports_unavailable_without_metadata() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let initialize = client.initialize(json!({}));
    assert_eq!(
        initialize["result"]["capabilities"],
        json!({ "tools": {}, "prompts": {}, "resources": {} })
    );
    assert!(initialize["result"]["capabilities"].get("tasks").is_none());
    assert!(!env.log_dir.join("stdin.txt").exists());

    let doctor = client.tool("doctor", json!({"cwd": env.root}));
    assert_eq!(
        doctor["taskExtensionReadiness"]["classification"],
        "unavailable"
    );
    assert_eq!(
        doctor["taskExtensionReadiness"]["serverAdvertisesTasks"],
        false
    );
    assert_eq!(doctor["taskExtensionReadiness"]["source"], "initialize");
    assert_eq!(doctor["summary"]["status"], "ok");
    assert_eq!(client.tool("agent_list", json!({}))["agents"], json!([]));
}

#[test]
fn stdio_agent_extension_readiness_reports_current_extension_metadata() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    client.initialize(json!({
        "capabilities": {
            "experimental": {
                "io.modelcontextprotocol/tasks": {
                    "version": "2026-03-26"
                }
            }
        }
    }));

    let doctor = client.tool("doctor", json!({"cwd": env.root}));
    assert_eq!(
        doctor["taskExtensionReadiness"]["classification"],
        "extension_capable"
    );
    assert_eq!(
        doctor["taskExtensionReadiness"]["observedExtensionIdentifiers"],
        json!(["io.modelcontextprotocol/tasks"])
    );
    assert_eq!(
        doctor["taskExtensionReadiness"]["serverAdvertisesTasks"],
        false
    );
    assert!(
        doctor["taskExtensionReadiness"]["recommendedNextStep"]
            .as_str()
            .unwrap()
            .contains("agent_*")
    );
    assert_eq!(doctor["summary"]["status"], "ok");
    assert_eq!(client.tool("agent_list", json!({}))["agents"], json!([]));
}

#[test]
fn stdio_agent_extension_readiness_reports_legacy_unknown_and_conflict_metadata() {
    let env = fixture_env();

    let mut legacy = McpClient::start(&env);
    legacy.initialize(json!({
        "capabilities": {
            "tasks": {
                "list": true,
                "cancel": true
            }
        }
    }));
    let legacy_doctor = legacy.tool("doctor", json!({"cwd": env.root}));
    assert_eq!(
        legacy_doctor["taskExtensionReadiness"]["classification"],
        "legacy_only"
    );
    assert!(
        legacy_doctor["taskExtensionReadiness"]["legacyIndicators"]
            .as_array()
            .unwrap()
            .iter()
            .any(|indicator| indicator.as_str().unwrap().contains("capabilities.tasks"))
    );
    drop(legacy);

    let mut unknown = McpClient::start(&env);
    unknown.initialize(json!({
        "capabilities": {
            "experimental": {
                "taskQueue": true
            }
        }
    }));
    let unknown_doctor = unknown.tool("doctor", json!({"cwd": env.root}));
    assert_eq!(
        unknown_doctor["taskExtensionReadiness"]["classification"],
        "unknown"
    );
    assert!(
        serde_json::to_string(&unknown_doctor)
            .unwrap()
            .contains("taskQueue")
    );
    drop(unknown);

    let mut conflict = McpClient::start(&env);
    conflict.initialize(json!({
        "capabilities": {
            "tasks": {"list": true},
            "experimental": {
                "io.modelcontextprotocol/tasks": {"version": "2026-03-26"}
            }
        }
    }));
    let conflict_doctor = conflict.tool("doctor", json!({"cwd": env.root}));
    assert_eq!(
        conflict_doctor["taskExtensionReadiness"]["classification"],
        "extension_capable"
    );
}

#[test]
fn stdio_agent_extension_readiness_reads_request_meta_without_raw_metadata_leak() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    client.initialize(json!({}));
    let tools = client.request(
        "tools/list",
        json!({
            "_meta": {
                "capabilities": {
                    "experimental": {
                        "io.modelcontextprotocol/tasks": {
                            "version": "2026-03-26",
                            "rawSecret": "task-extension-secret"
                        }
                    }
                }
            }
        }),
    );
    assert!(tools["result"]["tools"].is_array());
    assert!(!env.log_dir.join("stdin.txt").exists());

    let doctor = client.tool("doctor", json!({"cwd": env.root}));
    assert_eq!(
        doctor["taskExtensionReadiness"]["classification"],
        "extension_capable"
    );
    assert_eq!(doctor["taskExtensionReadiness"]["source"], "request_meta");
    let serialized = serde_json::to_string(&doctor).unwrap();
    assert!(!serialized.contains("task-extension-secret"));
    assert!(!env.state_dir.join("registry.json").exists());
}

#[test]
fn stdio_protocol_agent_methods_remain_unsupported() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);
    client.initialize(json!({
        "capabilities": {
            "experimental": {
                "io.modelcontextprotocol/tasks": {"version": "2026-03-26"}
            }
        }
    }));

    for method in [
        "tasks/get",
        "tasks/list",
        "tasks/cancel",
        "tasks/update",
        "tasks/result",
    ] {
        let response = client.request(method, json!({}));
        assert_eq!(response["error"]["code"], -32601, "{method}: {response}");
    }

    let tools = client.request("tools/list", json!({}));
    assert!(
        tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .all(|tool| !tool["name"].as_str().unwrap().starts_with("tasks/"))
    );
}

#[test]
fn stdio_tools_call_accepts_codex_meta_envelope() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let providers = client.tool_with_meta(
        "providers_list",
        json!({}),
        json!({"progressToken": "codex-live-call"}),
    );
    assert_eq!(
        providers["providers"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["claude", "cursor", "kimi", "codex", "forge", "antigravity"]
    );

    let preview = client.tool_with_meta(
        "agent_spawn",
        json!({
            "provider": "cursor",
            "mode": "review",
            "prompt": "secret prompt",
            "cwd": env.root
        , "dryRun": true}),
        json!({"ignored": {"provider": "claude", "maxTurns": 99}}),
    );
    assert_eq!(
        preview["command"].as_str().unwrap(),
        env.fake_provider.to_string_lossy()
    );
}

#[test]
fn stdio_tools_call_keeps_envelope_and_argument_validation_strict() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let unknown_envelope = client.tool_response_with_params(json!({
        "name": "providers_list",
        "arguments": {},
        "unexpectedEnvelopeField": true
    }));
    assert_eq!(unknown_envelope["result"]["isError"], true);
    assert!(
        unknown_envelope["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("unexpectedEnvelopeField")
    );

    let unknown_argument = client.tool_response_with_params(json!({
        "name": "agent_spawn",
        "_meta": {"progressToken": "codex-live-call"},
        "arguments": {
            "provider": "codex",
            "mode": "review",
            "prompt": "x",
            "cwd": env.root,
            "maxTurns": 2
        }
    }));
    assert_eq!(unknown_argument["result"]["isError"], true);
    assert!(
        unknown_argument["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Unknown argument")
    );

    let unknown_doctor_argument = client.tool_response_with_params(json!({
        "name": "doctor",
        "arguments": {
            "smoke": true,
            "maxTurns": 2
        }
    }));
    assert_eq!(unknown_doctor_argument["result"]["isError"], true);
    assert!(
        unknown_doctor_argument["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Unknown argument for doctor: maxTurns")
    );
}

#[test]
fn stdio_providers_preview_and_safety_checks() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let providers = client.tool("providers_list", json!({}));
    assert_eq!(
        providers["providers"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["claude", "cursor", "kimi", "codex", "forge", "antigravity"]
    );
    assert_eq!(
        providers["providers"]["codex"]["launchProfiles"],
        json!(["bridge", "bare"])
    );
    assert_eq!(
        providers["providers"]["claude"]["launchProfiles"],
        json!(["bridge", "bare", "unblocked"])
    );
    assert_eq!(
        providers["providers"]["codex"]["effort"],
        json!(["low", "medium", "high", "xhigh"])
    );
    assert_eq!(
        providers["providers"]["codex"]["reducedConfiguration"]["configIsolation"],
        "supported"
    );
    assert_eq!(
        providers["providers"]["codex"]["presentationActions"]["inspectResult"],
        "supported"
    );
    assert_eq!(
        providers["providers"]["codex"]["presentationActions"]["reply"],
        "unsupported"
    );
    assert_eq!(
        providers["providers"]["codex"]["readiness"]["state"],
        "stale"
    );
    assert_eq!(
        providers["providers"]["codex"]["readiness"]["launchable"],
        false
    );
    assert_eq!(
        providers["providers"]["cursor"]["reducedConfiguration"]["customSystemPrompt"],
        "unsupported"
    );
    assert_eq!(
        providers["providers"]["antigravity"]["reducedConfiguration"]["hooks"],
        "unsupported"
    );
    assert_eq!(
        providers["providers"]["antigravity"]["launchProfiles"],
        json!(["bridge", "bare", "unblocked"])
    );
    assert_eq!(
        providers["providers"]["antigravity"]["readOnlyEnforcement"]["review"],
        "prompt_enforced"
    );

    let checks = client.tool(
        "doctor",
        json!({"focus": "providers", "smoke": true, "timeoutMs": 5000}),
    );
    assert_eq!(
        checks["providers"]["codex"]["version"],
        "fake-provider 1.0.0"
    );
    assert_eq!(checks["providers"]["claude"]["startupVerified"], true);

    let preview = client.tool(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "secret prompt",
            "cwd": env.root,
            "profile": "bare"
        , "dryRun": true}),
    );
    assert_eq!(
        preview["command"].as_str().unwrap(),
        env.fake_provider.to_string_lossy()
    );
    assert_eq!(preview["timeoutSeconds"], 120);
    assert_eq!(preview["commandKind"], "acp");
    assert_eq!(preview["launchStrategy"], "acp");
    assert_eq!(preview["profile"], "bare");
    assert_eq!(preview["promptStrategy"], "compact");
    assert_eq!(
        preview["profileDiagnostics"]["appliedReductions"],
        json!([
            "compact_prompt",
            "ignore_user_config",
            "ignore_rules",
            "ephemeral_session"
        ])
    );
    assert_eq!(preview["args"], json!([]));
    assert_eq!(preview["stdin"], "<prompt redacted>");

    let codex_effort_preview = client.tool(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "codex effort prompt",
            "cwd": env.root,
            "effort": "high",
            "dryRun": true
        }),
    );
    assert_eq!(codex_effort_preview["commandKind"], "acp");

    let forge_preview = client.tool(
        "agent_spawn",
        json!({
            "provider": "forge",
            "mode": "review",
            "prompt": "secret forge prompt",
            "cwd": env.root,
            "profile": "bare"
        , "dryRun": true}),
    );
    assert_eq!(
        forge_preview["command"].as_str().unwrap(),
        env.fake_provider.to_string_lossy()
    );
    assert_eq!(forge_preview["commandKind"], "acp");
    assert_eq!(forge_preview["launchStrategy"], "acp");
    assert_eq!(forge_preview["profile"], "bare");
    assert_eq!(forge_preview["promptStrategy"], "compact");
    assert_eq!(forge_preview["args"], json!([]));
    assert_eq!(forge_preview["stdin"], "<prompt redacted>");

    let claude = client.tool(
        "agent_spawn",
        json!({
            "provider": "claude",
            "mode": "review",
            "prompt": "provider prompt",
            "cwd": env.root,
            "effort": "high"
        , "dryRun": true}),
    );
    assert_eq!(
        claude["command"].as_str().unwrap(),
        env.fake_provider.to_string_lossy()
    );
    assert_eq!(claude["commandKind"], "acp");
    assert_eq!(claude["launchStrategy"], "acp");
    assert!(
        claude["envKeys"]
            .as_array()
            .unwrap()
            .iter()
            .all(|key| key != "ANTHROPIC_BASE_URL")
    );
    assert!(
        claude["envKeys"]
            .as_array()
            .unwrap()
            .iter()
            .any(|key| key == "ANTHROPIC_API_KEY")
    );
    assert!(
        claude["envKeys"]
            .as_array()
            .unwrap()
            .iter()
            .any(|key| key == "ANTHROPIC_AUTH_TOKEN")
    );
    assert!(
        claude["envKeys"]
            .as_array()
            .unwrap()
            .iter()
            .any(|key| key == "CLAUDE_CODE_OAUTH_TOKEN")
    );

    let claude_unblocked = client.tool(
        "agent_spawn",
        json!({
            "provider": "claude",
            "mode": "implement",
            "prompt": "unblocked prompt",
            "cwd": env.root,
            "profile": "unblocked",
            "dryRun": true
        }),
    );
    assert_eq!(claude_unblocked["profile"], "unblocked");
    assert_eq!(claude_unblocked["promptStrategy"], "unblocked");
    assert_eq!(
        claude_unblocked["args"],
        json!(["--permission-mode", "bypassPermissions"])
    );
    assert_eq!(
        claude_unblocked["profileDiagnostics"]["permissionBypass"],
        "--permission-mode bypassPermissions"
    );

    let antigravity_preview = client.tool(
        "agent_spawn",
        json!({
            "provider": "antigravity",
            "mode": "review",
            "prompt": "secret antigravity prompt",
            "cwd": env.root,
            "model": "gemini-test",
            "profile": "bare",
            "timeoutSeconds": 7
        , "dryRun": true}),
    );
    assert_eq!(
        antigravity_preview["command"].as_str().unwrap(),
        env.fake_provider.to_string_lossy()
    );
    assert_eq!(antigravity_preview["timeoutSeconds"], 7);
    assert_eq!(antigravity_preview["commandKind"], "acp");
    assert_eq!(antigravity_preview["launchStrategy"], "acp");
    assert_eq!(antigravity_preview["profile"], "bare");
    assert_eq!(antigravity_preview["args"], json!([]));
    assert_eq!(antigravity_preview["stdin"], "<prompt redacted>");
    assert_eq!(
        antigravity_preview["profileDiagnostics"]["unsupportedReductions"],
        json!(["custom_system_prompt", "disable_hooks", "disable_skills"])
    );

    let antigravity_unblocked = client.tool(
        "agent_spawn",
        json!({
            "provider": "antigravity",
            "mode": "implement",
            "prompt": "unblocked antigravity prompt",
            "cwd": env.root,
            "profile": "unblocked",
            "dryRun": true
        }),
    );
    assert_eq!(
        antigravity_unblocked["args"],
        json!(["--dangerously-skip-permissions"])
    );
    assert_eq!(
        antigravity_unblocked["profileDiagnostics"]["permissionBypass"],
        "--dangerously-skip-permissions"
    );

    let codex_unblocked = client.tool_error(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "implement",
            "prompt": "unsupported unblocked prompt",
            "cwd": env.root,
            "profile": "unblocked",
            "dryRun": true
        }),
    );
    assert!(codex_unblocked.contains("codex does not support profile: unblocked"));

    let antigravity_implement = client.tool(
        "agent_spawn",
        json!({
            "provider": "antigravity",
            "mode": "implement",
            "prompt": "implement prompt",
            "cwd": env.root
        , "dryRun": true}),
    );
    assert!(
        !antigravity_implement["args"]
            .as_array()
            .unwrap()
            .contains(&json!("--sandbox"))
    );

    let outside = temp_dir("agent-bridge-outside");
    let link = env.root.join("escape");
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    let error = client.tool_error(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "x",
            "cwd": link
        , "dryRun": true}),
    );
    assert!(error.contains("outside configured workspaces"));

    let error = client.tool_error(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "x".repeat(101 * 1024),
            "cwd": env.root
        , "dryRun": true}),
    );
    assert!(error.contains("prompt exceeds"));
}

#[test]
fn stdio_provider_discovery_is_non_blocking_until_explicit_check() {
    let env = fixture_env();
    write_fake_provider(
        &env,
        &[
            "#!/bin/sh",
            "if [ -n \"$AGENT_BRIDGE_STATE_DIR\" ]; then",
            "  mkdir -p \"$AGENT_BRIDGE_STATE_DIR/provider-log\"",
            "  printf 'invoked\\n' >> \"$AGENT_BRIDGE_STATE_DIR/provider-log/discovery.txt\"",
            "fi",
            "case \"$*\" in",
            "  *--version*)",
            "  echo fake-provider 1.0.0",
            "  exit 0",
            "    ;;",
            "esac",
            "read init || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{}}}'",
            "read new_session || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"sessionId\":\"test-session\"}}'",
            "read prompt_request || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"test-session\",\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"AGENT_BRIDGE_PROVIDER_SMOKE_OK\"}}}}'",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"stopReason\":\"end_turn\"}}'",
            "exit 0",
            "",
        ],
    );
    let mut client = McpClient::start(&env);

    let providers = client.tool("providers_list", json!({}));

    assert_eq!(
        providers["providers"]["cursor"]["readiness"]["state"],
        "stale"
    );
    assert_eq!(
        providers["providers"]["cursor"]["readiness"]["launchable"],
        false
    );
    assert!(!env.log_dir.join("discovery.txt").exists());

    let checked = client.tool(
        "doctor",
        json!({"focus": "providers", "providers": ["cursor"]}),
    );
    assert_eq!(
        checked["providers"]["cursor"]["readiness"]["state"],
        "stale"
    );
    assert_eq!(checked["providers"]["cursor"]["launchable"], false);
    assert!(env.log_dir.join("discovery.txt").exists());

    let smoke = client.tool(
        "doctor",
        json!({"focus": "providers", "providers": ["cursor"], "smoke": true, "providerTimeoutMs": {"cursor": 5000}}),
    );
    assert_eq!(smoke["providers"]["cursor"]["readiness"]["state"], "ready");
    assert_eq!(smoke["providers"]["cursor"]["launchable"], true);
}

#[test]
fn stdio_agent_transcript_captures_redacted_events_and_result_evidence() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let task = client.tool(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "secret transcript prompt AGENT_BRIDGE_PROVIDER_SMOKE_OK",
            "cwd": env.root,
            "profile": "bare",
            "timeoutSeconds": 5
        }),
    );
    let agent_id = task["agentId"].as_str().unwrap().to_string();
    let completed = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": agent_id, "timeoutMs": 30000}),
    );
    assert_eq!(completed["status"], "succeeded");
    assert_eq!(completed["profile"], "bare");

    let transcript = client.tool(
        "agent_result",
        json!({"agentId": agent_id, "cursor": 0, "limit": 100, "sections": ["transcript"]}),
    )["transcript"]
        .clone();
    assert_eq!(transcript["available"], true);
    assert!(transcript["events"].as_array().unwrap().len() >= 3);
    assert!(transcript["nextCursor"].is_number());
    assert!(
        transcript["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["kind"] == "lifecycle" && event["parsed"]["phase"] == "spawned")
    );
    assert!(
        transcript["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["kind"] == "provider_result")
    );
    let serialized = serde_json::to_string(&transcript).unwrap();
    assert!(!serialized.contains("secret transcript prompt"));

    let first_page = client.tool(
        "agent_result",
        json!({"agentId": agent_id, "cursor": 0, "limit": 2, "sections": ["transcript"]}),
    )["transcript"]
        .clone();
    assert_eq!(first_page["events"].as_array().unwrap().len(), 2);
    assert_eq!(first_page["nextCursor"], 2);
    assert_eq!(first_page["truncated"], true);
    let second_page = client.tool("agent_result", json!({"agentId": agent_id, "cursor": first_page["nextCursor"], "limit": 100, "sections": ["transcript"]}))["transcript"].clone();
    assert!(!second_page["events"].as_array().unwrap().is_empty());

    let result = client.tool("agent_result", json!({"agentId": agent_id}));
    assert_eq!(result["reviewPacket"]["transcriptAvailable"], true);
    assert_eq!(result["reviewPacket"]["finalResultDetected"], true);
    assert_eq!(result["reviewPacket"]["partialResultDetected"], false);
    assert_eq!(result["reviewPacket"]["profile"], "bare");
    assert!(
        review_actions_text(&result).contains("transcript"),
        "{result}"
    );
}

#[test]
fn stdio_agent_transcript_reports_missing_artifact_without_hiding_logs() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let task = client.tool(
        "agent_spawn",
        json!({
            "provider": "cursor",
            "mode": "review",
            "prompt": "emit-logs",
            "cwd": env.root,
            "timeoutSeconds": 5
        }),
    );
    let agent_id = task["agentId"].as_str().unwrap().to_string();
    let completed = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": agent_id, "timeoutMs": 30000}),
    );
    assert_eq!(completed["status"], "succeeded");

    let agent_dir = env.state_dir.join("tasks").join(&agent_id);
    std::fs::remove_file(agent_dir.join("transcript.jsonl")).unwrap();

    let transcript = client.tool(
        "agent_result",
        json!({"agentId": agent_id, "sections": ["transcript"]}),
    )["transcript"]
        .clone();
    assert_eq!(transcript["available"], false);
    assert!(transcript["events"].as_array().unwrap().is_empty());
    assert!(
        transcript["message"]
            .as_str()
            .unwrap()
            .contains("not available")
    );

    let logs = client.tool(
        "agent_result",
        json!({"agentId": agent_id, "sections": ["stdout", "stderr"]}),
    );
    assert!(
        logs["stdout"]
            .as_str()
            .unwrap()
            .contains("lifecycle-stdout")
    );
}

#[test]
fn stdio_agent_transcript_preserves_raw_events_and_partial_evidence() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let task = client.tool(
        "agent_spawn",
        json!({
            "provider": "cursor",
            "mode": "review",
            "prompt": "malformed-json",
            "cwd": env.root,
            "timeoutSeconds": 5
        }),
    );
    let agent_id = task["agentId"].as_str().unwrap().to_string();
    let completed = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": agent_id, "timeoutMs": 30000}),
    );
    assert_eq!(completed["status"], "succeeded");

    let transcript = client.tool(
        "agent_result",
        json!({"agentId": agent_id, "sections": ["transcript"]}),
    )["transcript"]
        .clone();
    assert!(
        transcript["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["kind"] == "provider_event"
                && event["raw"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("{\"type\":"))
    );

    let result = client.tool("agent_result", json!({"agentId": agent_id}));
    assert_eq!(result["reviewPacket"]["transcriptAvailable"], true);
    assert_eq!(result["reviewPacket"]["finalResultDetected"], true);
    assert_eq!(result["reviewPacket"]["partialResultDetected"], false);
}

#[test]
fn stdio_agent_transcript_redacts_provider_env_values() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let task = client.tool(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "echo-api-key",
            "cwd": env.root,
            "timeoutSeconds": 5
        }),
    );
    let agent_id = task["agentId"].as_str().unwrap().to_string();
    let completed = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": agent_id, "timeoutMs": 30000}),
    );
    assert_eq!(completed["status"], "succeeded");

    let transcript = client.tool(
        "agent_result",
        json!({"agentId": agent_id, "sections": ["transcript"]}),
    )["transcript"]
        .clone();
    let serialized = serde_json::to_string(&transcript).unwrap();
    assert!(!serialized.contains("test-key"));
    assert!(serialized.contains("<redacted>"));
}

#[test]
fn stdio_agent_result_preserves_final_result_evidence_after_timeout() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let task = client.tool(
        "agent_spawn",
        json!({
            "provider": "cursor",
            "mode": "review",
            "prompt": "final-then-timeout",
            "cwd": env.root,
            "timeoutSeconds": 1
        }),
    );
    let agent_id = task["agentId"].as_str().unwrap().to_string();
    let completed = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": agent_id, "timeoutMs": 30000}),
    );
    assert_eq!(completed["status"], "failed");
    assert_eq!(completed["errorType"], "timeout");

    let result = client.tool("agent_result", json!({"agentId": agent_id}));
    assert_eq!(result["reviewPacket"]["transcriptAvailable"], true);
    assert_eq!(result["reviewPacket"]["finalResultDetected"], false);
    assert_eq!(result["reviewPacket"]["partialResultDetected"], true);
}

#[test]
fn stdio_agent_transcript_handles_provider_output_fixtures() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let provider = "cursor";
    let task = client.tool(
        "agent_spawn",
        json!({
            "provider": provider,
            "mode": "review",
            "prompt": "AGENT_BRIDGE_PROVIDER_SMOKE_OK",
            "cwd": env.root,
            "timeoutSeconds": 5
        }),
    );
    let agent_id = task["agentId"].as_str().unwrap().to_string();
    let completed = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": agent_id, "timeoutMs": 30000}),
    );
    assert_eq!(completed["status"], "succeeded", "{provider}");
    let result = client.tool("agent_result", json!({"agentId": agent_id}));
    assert_eq!(
        result["reviewPacket"]["finalResultDetected"], true,
        "{provider}"
    );
    assert_eq!(
        result["reviewPacket"]["partialResultDetected"], false,
        "{provider}"
    );

    let task = client.tool(
        "agent_spawn",
        json!({
            "provider": "kimi",
            "mode": "review",
            "prompt": "emit-logs",
            "cwd": env.root,
            "timeoutSeconds": 5
        }),
    );
    let agent_id = task["agentId"].as_str().unwrap().to_string();
    let completed = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": agent_id, "timeoutMs": 30000}),
    );
    assert_eq!(completed["status"], "succeeded");
    let result = client.tool("agent_result", json!({"agentId": agent_id}));
    assert_eq!(result["reviewPacket"]["finalResultDetected"], true);
    assert_eq!(result["reviewPacket"]["partialResultDetected"], false);
}

#[test]
fn stdio_doctor_default_report_shape_and_side_effects() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    assert_eq!(client.tool("agent_list", json!({}))["agents"], json!([]));

    let doctor = client.tool("doctor", json!({}));
    for section in [
        "summary",
        "server",
        "workspace",
        "state",
        "binary",
        "clients",
        "taskExtensionReadiness",
        "providers",
        "launchReadiness",
        "claudeHostRunner",
        "recommendations",
    ] {
        assert!(
            doctor.get(section).is_some(),
            "missing doctor section: {section}"
        );
    }
    assert!(matches!(
        doctor["summary"]["status"].as_str(),
        Some("ok" | "warning" | "error")
    ));
    assert_eq!(doctor["server"]["name"], "agent-bridge-mcp");
    assert_eq!(doctor["server"]["protocolVersion"], "2024-11-05");
    assert!(matches!(
        doctor["binary"]["status"].as_str(),
        Some("ok" | "warning" | "error" | "unknown")
    ));
    assert!(doctor["binary"]["running"]["path"].is_string());
    assert_eq!(doctor["clients"]["codex"]["configPresent"], false);
    assert_eq!(doctor["clients"]["codex"]["registrationStatus"], "absent");
    assert_eq!(doctor["clients"]["claude"]["configPresent"], false);
    assert_eq!(doctor["clients"]["cursor"]["configPresent"], false);
    assert_eq!(doctor["providers"]["claude"]["startupVerified"], false);
    assert_eq!(doctor["providers"]["codex"]["startupVerified"], false);
    assert_eq!(doctor["launchReadiness"]["startupVerified"], false);
    assert_eq!(doctor["claudeHostRunner"]["status"], "not_configured");
    assert_eq!(
        doctor["claudeHostRunner"]["launchStrategy"],
        "host_runner_required"
    );
    assert!(doctor["recommendations"].is_array());

    assert_eq!(client.tool("agent_list", json!({}))["agents"], json!([]));
    assert!(!env.log_dir.join("stdin.txt").exists());
}

#[test]
fn stdio_doctor_reports_binary_freshness_with_overrides_without_mutation() {
    let env = fixture_env();
    let installed = env.root.join("installed-agent-bridge-mcp");
    let release = env.root.join("target/release/agent-bridge-mcp");
    std::fs::create_dir_all(release.parent().unwrap()).unwrap();
    std::fs::write(&installed, "old-binary").unwrap();
    std::fs::write(&release, "new-binary").unwrap();
    let installed_before = std::fs::metadata(&installed).unwrap();
    let release_before = std::fs::metadata(&release).unwrap();

    let mut extra_env = BTreeMap::new();
    extra_env.insert(
        "AGENT_BRIDGE_INSTALLED_BIN".to_string(),
        OsString::from(&installed),
    );
    extra_env.insert(
        "AGENT_BRIDGE_RELEASE_BIN".to_string(),
        OsString::from(&release),
    );
    let mut client = McpClient::start_with_extra_env(&env, extra_env);

    let doctor = client.tool("doctor", json!({"cwd": env.root}));
    assert_eq!(doctor["binary"]["status"], "warning");
    assert_eq!(
        doctor["binary"]["installed"]["path"],
        installed.display().to_string()
    );
    assert_eq!(
        doctor["binary"]["release"]["path"],
        release.display().to_string()
    );
    assert_eq!(doctor["binary"]["installed"]["matchesRelease"], false);
    assert_eq!(doctor["summary"]["status"], "ok");
    assert!(
        doctor["binary"]["recommendations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|recommendation| recommendation
                .as_str()
                .unwrap()
                .contains("Rebuild and install"))
    );
    assert!(doctor["recommendations"].as_array().unwrap().iter().any(
        |recommendation| recommendation["id"] == "install_release_binary"
            && recommendation["command"][4] == installed.display().to_string()
    ));

    let installed_after = std::fs::metadata(&installed).unwrap();
    let release_after = std::fs::metadata(&release).unwrap();
    assert_eq!(installed_before.len(), installed_after.len());
    assert_eq!(release_before.len(), release_after.len());
    assert_eq!(std::fs::read_to_string(&installed).unwrap(), "old-binary");
    assert_eq!(std::fs::read_to_string(&release).unwrap(), "new-binary");
}

#[test]
fn stdio_doctor_binary_uses_cwd_release_path_without_override() {
    let env = fixture_env();
    let installed = env.root.join("installed-agent-bridge-mcp");
    let release = env.root.join("target/release/agent-bridge-mcp");
    std::fs::create_dir_all(release.parent().unwrap()).unwrap();
    std::fs::write(&installed, "same-binary").unwrap();
    std::fs::write(&release, "same-binary").unwrap();

    let mut extra_env = BTreeMap::new();
    extra_env.insert(
        "AGENT_BRIDGE_INSTALLED_BIN".to_string(),
        OsString::from(&installed),
    );
    let mut client = McpClient::start_with_extra_env(&env, extra_env);

    let doctor = client.tool("doctor", json!({"cwd": env.root}));
    assert_eq!(
        doctor["binary"]["release"]["path"],
        release.display().to_string()
    );
    assert_eq!(doctor["binary"]["installed"]["matchesRelease"], true);
    assert_eq!(doctor["binary"]["installed"]["fingerprintStatus"], "ok");
    assert_eq!(doctor["binary"]["release"]["fingerprintStatus"], "ok");
}

#[test]
fn stdio_doctor_redacts_secret_environment_values() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let doctor = client.tool("doctor", json!({"cwd": env.root}));
    let server = &doctor["server"];
    assert_eq!(server["environment"]["ANTHROPIC_API_KEY"]["present"], true);
    assert_eq!(
        server["environment"]["ANTHROPIC_AUTH_TOKEN"]["present"],
        true
    );
    assert_eq!(
        server["environment"]["CLAUDE_CODE_OAUTH_TOKEN"]["present"],
        true
    );
    assert_eq!(
        server["environment"]["ANTHROPIC_API_KEY"]["value"],
        "<redacted>"
    );
    assert!(
        doctor["recommendations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|recommendation| recommendation
                .get("arguments")
                .is_none_or(|arguments| arguments.is_object()))
    );

    let serialized = serde_json::to_string(&doctor).unwrap();
    assert!(!serialized.contains("test-key"));
    assert!(!serialized.contains("test-auth-token"));
    assert!(!serialized.contains("test-code-oauth-token"));
}

#[test]
fn stdio_doctor_reports_registered_client_configs_without_secret_values() {
    let env = fixture_env();
    let bridge_bin = env.root.join("agent-bridge-mcp");
    std::fs::write(&bridge_bin, "#!/bin/sh\n").unwrap();

    let codex_dir = env.root.join(".codex");
    let cursor_dir = env.root.join(".cursor");
    std::fs::create_dir_all(&codex_dir).unwrap();
    std::fs::create_dir_all(&cursor_dir).unwrap();
    std::fs::write(
        codex_dir.join("config.toml"),
        format!(
            r#"
[mcp_servers."agent-bridge"]
command = "{}"
args = ["--stdio"]
env = {{ AGENT_BRIDGE_WORKSPACES = "{}", ANTHROPIC_API_KEY = "codex-secret" }}
"#,
            bridge_bin.display(),
            env.root.display()
        ),
    )
    .unwrap();
    std::fs::write(
        env.root.join(".claude.json"),
        format!(
            r#"{{
  "mcpServers": {{
    "agent-bridge": {{
      "command": "{}",
      "args": ["--stdio"],
      "env": {{
        "AGENT_BRIDGE_WORKSPACES": "{}",
        "CLAUDE_CODE_OAUTH_TOKEN": "claude-secret"
      }}
    }}
  }}
}}"#,
            bridge_bin.display(),
            env.root.display()
        ),
    )
    .unwrap();
    std::fs::write(
        cursor_dir.join("mcp.json"),
        r#"{
  "mcpServers": {
    "agent-bridge": {
      "command": "agent-bridge-mcp",
      "env": {
        "CURSOR_TOKEN": "cursor-secret"
      }
    }
  }
}"#,
    )
    .unwrap();

    let mut client = McpClient::start(&env);
    let doctor = client.tool("doctor", json!({"cwd": env.root}));

    assert_eq!(doctor["clients"]["codex"]["status"], "ok");
    assert_eq!(
        doctor["clients"]["codex"]["command"]["resolution"],
        "absolute_exists"
    );
    assert_eq!(
        doctor["clients"]["codex"]["envKeys"],
        json!(["AGENT_BRIDGE_WORKSPACES", "ANTHROPIC_API_KEY"])
    );
    assert_eq!(
        doctor["clients"]["codex"]["verificationCommands"][0]["command"],
        json!(["codex", "mcp", "list"])
    );
    assert_eq!(
        doctor["clients"]["claude"]["registrationStatus"],
        "registered"
    );
    assert_eq!(
        doctor["clients"]["claude"]["verificationCommands"][0]["command"],
        json!(["claude", "mcp", "list"])
    );
    assert_eq!(
        doctor["clients"]["cursor"]["command"]["resolution"],
        "path_lookup_required"
    );
    assert_eq!(
        doctor["clients"]["cursor"]["verificationCommands"],
        json!([])
    );

    let recommendations = doctor["recommendations"].as_array().unwrap();
    assert!(recommendations.iter().any(|recommendation| {
        recommendation["kind"] == "shell"
            && recommendation["command"] == json!(["codex", "mcp", "list"])
    }));
    assert!(recommendations.iter().any(|recommendation| {
        recommendation["kind"] == "shell"
            && recommendation["command"] == json!(["claude", "mcp", "list"])
    }));

    let serialized = serde_json::to_string(&doctor).unwrap();
    assert!(!serialized.contains("codex-secret"));
    assert!(!serialized.contains("claude-secret"));
    assert!(!serialized.contains("cursor-secret"));
}

#[test]
fn stdio_doctor_reports_client_config_absent_malformed_and_missing_command() {
    let env = fixture_env();
    std::fs::create_dir_all(env.root.join(".codex")).unwrap();
    std::fs::create_dir_all(env.root.join(".cursor")).unwrap();
    std::fs::write(
        env.root.join(".codex/config.toml"),
        "[mcp_servers.other]\ncommand = \"agent-bridge-mcp\"\n",
    )
    .unwrap();
    std::fs::write(env.root.join(".claude.json"), "{not-json").unwrap();
    std::fs::write(
        env.root.join(".cursor/mcp.json"),
        r#"{"mcpServers":{"agent-bridge":{"env":{"TOKEN":"secret"}}}}"#,
    )
    .unwrap();

    let mut client = McpClient::start(&env);
    let doctor = client.tool("doctor", json!({"cwd": env.root}));

    assert_eq!(doctor["clients"]["codex"]["parseStatus"], "ok");
    assert_eq!(doctor["clients"]["codex"]["registrationStatus"], "absent");
    assert_eq!(doctor["clients"]["codex"]["status"], "info");
    assert_eq!(doctor["clients"]["claude"]["parseStatus"], "error");
    assert_eq!(doctor["clients"]["claude"]["status"], "error");
    assert_eq!(
        doctor["clients"]["cursor"]["registrationStatus"],
        "registered"
    );
    assert_eq!(
        doctor["clients"]["cursor"]["command"]["resolution"],
        "missing"
    );
    assert_eq!(doctor["clients"]["cursor"]["status"], "warning");
    assert_eq!(doctor["summary"]["status"], "ok");

    let serialized = serde_json::to_string(&doctor).unwrap();
    assert!(!serialized.contains("secret"));
}

#[test]
fn stdio_doctor_summary_status_reflects_errors_and_warnings() {
    let env = fixture_env();

    let mut ok_client = McpClient::start(&env);
    let ok = ok_client.tool("doctor", json!({"cwd": env.root}));
    assert_eq!(ok["summary"]["status"], "ok");
    drop(ok_client);

    let mut warning_env = BTreeMap::new();
    warning_env.insert(
        "CODEX_ACP_BIN".to_string(),
        OsString::from("/missing/codex"),
    );
    let mut warning_client = McpClient::start_with_extra_env(&env, warning_env);
    let warning = warning_client.tool("doctor", json!({"cwd": env.root, "providers": ["codex"]}));
    assert_eq!(warning["summary"]["status"], "warning");
    drop(warning_client);

    let mut error_client = McpClient::start_without_workspace(&env);
    let error = error_client.tool("doctor", json!({"cwd": env.root}));
    assert_eq!(error["summary"]["status"], "error");
    assert!(
        error["recommendations"][0]["message"]
            .as_str()
            .unwrap()
            .contains("AGENT_BRIDGE_WORKSPACES")
    );
}

#[test]
fn stdio_doctor_reads_workspace_policy_from_config_file() {
    let env = fixture_env();
    let config_dir = env.root.join(".agent-bridge-mcp");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.toml"),
        format!("workspaces = [\"{}\"]\n", env.root.display()),
    )
    .unwrap();

    let mut client = McpClient::start_without_workspace(&env);
    let report = client.tool("doctor", json!({"cwd": env.root}));

    assert_eq!(report["workspace"]["status"], "ok");
    assert_eq!(
        report["workspace"]["cwd"]["insideConfiguredWorkspace"],
        true
    );
}

#[test]
fn stdio_doctor_reports_workspace_diagnostic_errors() {
    let env = fixture_env();
    let outside = temp_dir("agent-bridge-doctor-outside");
    let mut client = McpClient::start(&env);
    let outside_report = client.tool("doctor", json!({"cwd": outside}));
    assert_eq!(outside_report["workspace"]["status"], "error");
    assert!(
        outside_report["workspace"]["cwd"]["error"]
            .as_str()
            .unwrap()
            .contains("outside configured workspaces")
    );

    let missing = env.root.join("missing-cwd");
    let invalid_cwd = client.tool("doctor", json!({"cwd": missing}));
    assert_eq!(invalid_cwd["workspace"]["status"], "error");
    assert_eq!(
        invalid_cwd["workspace"]["cwd"]["insideConfiguredWorkspace"],
        false
    );
    drop(client);

    let invalid_root = env.root.join("missing-root");
    let mut invalid_client =
        McpClient::start_with_workspace_value(&env, OsString::from(invalid_root));
    let invalid_root_report = invalid_client.tool("doctor", json!({"cwd": env.root}));
    assert_eq!(invalid_root_report["workspace"]["status"], "error");
}

#[test]
fn stdio_doctor_reports_state_dir_creation_and_registry_errors() {
    let env = fixture_env();
    let missing_state_dir = env.root.join("new-state-dir");
    let mut extra_env = BTreeMap::new();
    extra_env.insert(
        "AGENT_BRIDGE_STATE_DIR".to_string(),
        OsString::from(&missing_state_dir),
    );
    let mut client = McpClient::start_with_extra_env(&env, extra_env);
    let created = client.tool("doctor", json!({"cwd": env.root}));
    assert_eq!(created["state"]["status"], "ok");
    assert!(missing_state_dir.is_dir());

    std::fs::write(env.state_dir.join("registry.json"), "{not-json").unwrap();
    let mut invalid_client = McpClient::start(&env);
    let invalid = invalid_client.tool("doctor", json!({"cwd": env.root}));
    assert_eq!(invalid["state"]["status"], "error");
    assert!(
        invalid["state"]["error"]
            .as_str()
            .unwrap()
            .contains("registry")
    );
    drop(invalid_client);

    std::fs::write(
        env.state_dir.join("registry.json"),
        serde_json::to_string(&json!({
            "tasks": {
                "agent_missing_fields": {
                    "agentId": "agent_missing_fields"
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();
    let mut typed_invalid_client = McpClient::start(&env);
    let typed_invalid = typed_invalid_client.tool("doctor", json!({"cwd": env.root}));
    assert_eq!(typed_invalid["state"]["status"], "error");
    assert!(
        typed_invalid["state"]["error"]
            .as_str()
            .unwrap()
            .contains("missing field")
    );
}

#[test]
fn stdio_doctor_uses_runtime_default_state_dir_when_unset() {
    let env = fixture_env();
    let mut client = McpClient::start_without_state_dir(&env);
    let report = client.tool("doctor", json!({"cwd": env.root}));
    let expected = env.root.join(".agent-bridge-mcp").join("state");

    assert_eq!(report["state"]["status"], "ok");
    assert_eq!(report["state"]["path"], expected.display().to_string());
    assert!(expected.is_dir());
}

#[test]
fn stdio_doctor_orders_recommendations_from_blockers_to_followups() {
    let env = fixture_env();
    std::fs::write(env.state_dir.join("registry.json"), "{not-json").unwrap();
    let mut extra_env = BTreeMap::new();
    extra_env.insert(
        "AGENT_BRIDGE_CLAUDE_HOST_SOCKET".to_string(),
        OsString::from(env.root.join("missing-host.sock")),
    );
    extra_env.insert(
        "CODEX_ACP_BIN".to_string(),
        OsString::from("/missing/codex"),
    );
    let mut client = McpClient::start_with_extra_env(&env, extra_env);

    let report = client.tool("doctor", json!({"cwd": env.root, "providers": ["codex"]}));
    let messages: Vec<_> = report["recommendations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|recommendation| recommendation["message"].as_str().unwrap())
        .collect();

    assert!(messages[0].contains("AGENT_BRIDGE_STATE_DIR"));
    assert!(messages[1].contains("host runner"));
    assert!(messages[2].contains("providers"));
    assert!(
        messages
            .iter()
            .skip(3)
            .any(|message| message.contains("user-level MCP config"))
    );
}

#[test]
fn stdio_doctor_reuses_provider_readiness_controls() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let antigravity = client.tool(
        "doctor",
        json!({"cwd": env.root, "providers": ["antigravity"]}),
    );
    assert_eq!(
        provider_keys(&json!({"providers": antigravity["providers"]})),
        vec!["antigravity"]
    );
    assert_eq!(
        antigravity["providers"]["antigravity"]["version"],
        "fake-provider 1.0.0"
    );
    assert_eq!(
        antigravity["server"]["environment"]["AGY_BIN"]["present"],
        true
    );
    assert_eq!(antigravity["summary"]["status"], "ok");

    let default = client.tool("doctor", json!({"cwd": env.root, "providers": ["codex"]}));
    assert_eq!(
        provider_keys(&json!({"providers": default["providers"]})),
        vec!["codex"]
    );
    assert_eq!(
        default["providers"]["codex"]["version"],
        "fake-provider 1.0.0"
    );
    assert_eq!(default["providers"]["codex"]["startupVerified"], false);
    assert_eq!(default["summary"]["status"], "ok");
    assert_eq!(default["launchReadiness"]["status"], "not_verified");
    let smoke_recommendation = default["recommendations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|recommendation| recommendation["id"] == "verify_provider_startup")
        .expect("doctor should recommend startup smoke for selected stale provider");
    assert_eq!(smoke_recommendation["tool"], "doctor");
    assert_eq!(smoke_recommendation["arguments"]["focus"], "providers");
    assert_eq!(
        smoke_recommendation["arguments"]["providers"],
        json!(["codex"])
    );
    assert_eq!(smoke_recommendation["arguments"]["smoke"], true);
    assert!(
        default["providers"]["codex"]
            .get("smokeDurationMs")
            .is_none()
    );

    let smoke = client.tool(
        "doctor",
        json!({"cwd": env.root, "providers": ["codex"], "smoke": true, "aggregateTimeoutMs": 5000, "providerTimeoutMs": {"codex": 5000}}),
    );
    assert_eq!(smoke["providers"]["codex"]["startupVerified"], true);
    assert_eq!(smoke["launchReadiness"]["status"], "ready");
    assert!(smoke["providers"]["codex"]["smokeDurationMs"].is_number());
}

#[test]
fn stdio_doctor_deduplicates_provider_filter_before_checking() {
    let env = fixture_env();
    write_fake_provider(
        &env,
        &[
            "#!/bin/sh",
            "stdin=$(cat)",
            "if [ -n \"$AGENT_BRIDGE_STATE_DIR\" ]; then",
            "  mkdir -p \"$AGENT_BRIDGE_STATE_DIR/provider-log\"",
            "  printf '%s\\n' \"$*\" >> \"$AGENT_BRIDGE_STATE_DIR/provider-log/argv.txt\"",
            "  printf '%s' \"$stdin\" > \"$AGENT_BRIDGE_STATE_DIR/provider-log/stdin.txt\"",
            "fi",
            "if [ \"$1\" = \"--version\" ]; then",
            "  echo fake-provider 1.0.0",
            "  exit 0",
            "fi",
            "printf '%s\\n' '{\"type\":\"result\",\"subtype\":\"success\",\"result\":\"AGENT_BRIDGE_PROVIDER_SMOKE_OK\"}'",
            "exit 0",
            "",
        ],
    );
    let mut client = McpClient::start(&env);

    let report = client.tool(
        "doctor",
        json!({"cwd": env.root, "providers": ["cursor", "cursor"]}),
    );
    assert_eq!(
        provider_keys(&json!({"providers": report["providers"]})),
        vec!["cursor"]
    );

    let argv = std::fs::read_to_string(env.log_dir.join("argv.txt")).unwrap();
    assert_eq!(argv.lines().filter(|line| *line == "--version").count(), 1);
}

#[test]
fn stdio_claude_host_runner_preview_and_unavailable_smoke_are_explicit() {
    let env = fixture_env();
    let mut extra_env = BTreeMap::new();
    extra_env.insert(
        "AGENT_BRIDGE_CLAUDE_HOST_SOCKET".to_string(),
        OsString::from(env.root.join("missing-host.sock")),
    );
    let mut client = McpClient::start_with_extra_env(&env, extra_env);

    let preview = client.tool(
        "agent_spawn",
        json!({
            "provider": "claude",
            "mode": "review",
            "prompt": "provider prompt",
            "cwd": env.root
        , "dryRun": true}),
    );
    assert_eq!(preview["launchStrategy"], "acp");

    let checks = client.tool(
        "doctor",
        json!({"focus": "providers", "providers": ["claude"], "smoke": true, "timeoutMs": 500}),
    );
    let claude = &checks["providers"]["claude"];
    assert_eq!(claude["available"], true);
    assert_eq!(claude["startupVerified"], true);
}

#[test]
fn stdio_doctor_reports_claude_host_runner_states() {
    let env = fixture_env();

    let mut missing_env = BTreeMap::new();
    missing_env.insert(
        "AGENT_BRIDGE_CLAUDE_HOST_SOCKET".to_string(),
        OsString::from(env.root.join("missing-host.sock")),
    );
    let mut missing_client = McpClient::start_with_extra_env(&env, missing_env);
    let started = Instant::now();
    let missing = missing_client.tool("doctor", json!({"cwd": env.root, "providers": ["codex"]}));
    assert_eq!(missing["claudeHostRunner"]["status"], "error");
    assert!(started.elapsed() < Duration::from_millis(1500));
}

#[test]
fn stdio_providers_check_filters_and_validates_readiness_inputs() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let filtered = client.tool(
        "doctor",
        json!({"focus": "providers", "smoke": false, "providers": ["cursor", "cursor"]}),
    );
    assert_eq!(provider_keys(&filtered), vec!["cursor"]);
    assert_eq!(
        filtered["providers"]["cursor"]["version"],
        "fake-provider 1.0.0"
    );
    assert!(filtered["providers"]["cursor"]["versionDurationMs"].is_number());
    assert!(filtered["providers"]["cursor"]["checkedAt"].is_string());
    assert_eq!(filtered["providers"]["cursor"]["launchable"], false);
    assert_eq!(
        filtered["providers"]["cursor"]["readiness"]["state"],
        "stale"
    );
    assert_eq!(
        filtered["providers"]["cursor"]["readiness"]["probe"],
        "version"
    );
    assert_eq!(
        filtered["providers"]["cursor"]["readiness"]["launchable"],
        false
    );
    assert!(
        filtered["providers"]["cursor"]
            .get("smokeDurationMs")
            .is_none()
    );

    let claude_version = client.tool(
        "doctor",
        json!({"focus": "providers", "smoke": false, "providers": ["claude"]}),
    );
    let claude = &claude_version["providers"]["claude"];
    assert_eq!(claude["version"], "fake-provider 1.0.0");
    assert_eq!(claude["startupVerified"], false);
    assert_eq!(claude["launchable"], false);
    assert_eq!(claude["readiness"]["state"], "stale");
    assert_eq!(claude["readiness"]["probe"], "version");
    assert_eq!(claude["readiness"]["launchable"], false);

    let unknown_provider = client.tool_error(
        "doctor",
        json!({"focus": "providers", "providers": ["openai"]}),
    );
    assert!(unknown_provider.contains("claude"));
    assert!(unknown_provider.contains("codex"));

    let empty_filter = client.tool_error("doctor", json!({"focus": "providers", "providers": []}));
    assert!(empty_filter.contains("at least one provider"));

    let invalid_aggregate = client.tool_error(
        "doctor",
        json!({"focus": "providers", "smoke": true, "aggregateTimeoutMs": 0}),
    );
    assert!(invalid_aggregate.contains("aggregateTimeoutMs"));

    let invalid_budget = client.tool_error(
        "doctor",
        json!({"focus": "providers", "smoke": true, "providerTimeoutMs": {"cursor": 0}}),
    );
    assert!(invalid_budget.contains("providerTimeoutMs.cursor"));

    let unknown_budget_provider = client.tool_error(
        "doctor",
        json!({"focus": "providers", "smoke": true, "providerTimeoutMs": {"openai": 1000}}),
    );
    assert!(unknown_budget_provider.contains("provider must be one of"));
}

#[test]
fn stdio_unblocked_provider_smoke_uses_profile_flags_and_workspace_probe() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let checks = client.tool(
        "doctor",
        json!({
            "focus": "providers",
            "smoke": true,
            "providers": ["claude"],
            "profile": "unblocked",
            "cwd": env.root,
            "providerTimeoutMs": {"claude": 5000}
        }),
    );

    let claude = &checks["providers"]["claude"];
    assert_eq!(claude["profile"], "unblocked");
    assert_eq!(claude["startupVerified"], true);
    assert_eq!(claude["launchable"], true);
    assert_eq!(claude["readiness"]["state"], "ready");
    assert_eq!(claude["smokePromptStrategy"], "unblocked");
    assert!(
        !env.root.join(".agent-bridge-unblocked-smoke").exists(),
        "smoke marker should be removed after the provider proves write/read/delete"
    );

    let argv = std::fs::read_to_string(env.log_dir.join("argv.txt")).unwrap();
    assert!(argv.contains("--permission-mode"));
    assert!(argv.contains("bypassPermissions"));
    let stdin = std::fs::read_to_string(env.log_dir.join("stdin.txt")).unwrap();
    assert!(stdin.contains(".agent-bridge-unblocked-smoke"));
}

#[test]
fn stdio_unblocked_provider_smoke_keeps_workspace_validation_authoritative() {
    let env = fixture_env();
    let outside = temp_dir("agent-bridge-outside");
    let mut client = McpClient::start(&env);

    let error = client.tool_error(
        "doctor",
        json!({
            "focus": "providers",
            "smoke": true,
            "providers": ["claude"],
            "profile": "unblocked",
            "cwd": outside
        }),
    );

    assert!(error.contains("cwd is outside configured workspaces"));
}

#[test]
fn stdio_antigravity_smoke_auth_failure_preserves_version_availability() {
    let env = fixture_env();
    write_fake_provider(
        &env,
        &[
            "#!/bin/sh",
            "if [ \"$1\" = \"--version\" ]; then",
            "  echo agy 1.0.0",
            "  exit 0",
            "fi",
            "printf 'Authentication required. Please visit the URL to log in.\\n' >&2",
            "exit 1",
            "",
        ],
    );
    let mut client = McpClient::start(&env);

    let checks = client.tool(
        "doctor",
        json!({"focus": "providers",
            "smoke": true,
            "providers": ["antigravity"],
            "providerTimeoutMs": {"antigravity": 1000}
        }),
    );
    let antigravity = &checks["providers"]["antigravity"];
    assert_eq!(antigravity["available"], true);
    assert_eq!(antigravity["version"], "agy 1.0.0");
    assert_eq!(antigravity["startupVerified"], false);
    assert_eq!(antigravity["launchable"], false);
    assert_eq!(antigravity["readiness"]["state"], "failed");
    assert_eq!(
        antigravity["diagnostic"]["failureCategory"],
        "provider_output_error"
    );
    assert!(
        antigravity["diagnostic"]["stderrExcerpt"]
            .as_str()
            .unwrap()
            .contains("Authentication required")
    );
}

#[test]
fn stdio_providers_check_uses_provider_budgets_and_concurrency() {
    let env = fixture_env();
    write_fake_provider(
        &env,
        &[
            "#!/bin/sh",
            "case \"$*\" in",
            "  *--version*)",
            "  echo fake-provider 1.0.0",
            "  exit 0",
            "    ;;",
            "esac",
            "read init || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{}}}'",
            "read new_session || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"sessionId\":\"test-session\"}}'",
            "read prompt_request || exit 1",
            "sleep 2",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"test-session\",\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"AGENT_BRIDGE_PROVIDER_SMOKE_OK\"}}}}'",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"stopReason\":\"end_turn\"}}'",
            "exit 0",
            "",
        ],
    );
    let mut client = McpClient::start(&env);
    let started = std::time::Instant::now();

    let checks = client.tool(
        "doctor",
        json!({"focus": "providers",
            "smoke": true,
            "providers": ["cursor", "kimi"],
            "aggregateTimeoutMs": 4500,
            "providerTimeoutMs": {"cursor": 3000, "kimi": 3000}
        }),
    );

    assert_eq!(sorted_provider_keys(&checks), vec!["cursor", "kimi"]);
    assert_eq!(checks["providers"]["cursor"]["startupVerified"], true);
    assert_eq!(checks["providers"]["kimi"]["startupVerified"], true);
    assert_eq!(checks["providers"]["cursor"]["launchable"], true);
    assert_eq!(checks["providers"]["cursor"]["readiness"]["state"], "ready");
    assert_eq!(
        checks["providers"]["cursor"]["readiness"]["probe"],
        "version+smoke"
    );
    assert!(checks["providers"]["cursor"]["smokeDurationMs"].is_number());
    assert!(
        started.elapsed() < std::time::Duration::from_millis(3600),
        "smoke probes should run concurrently: {:?}",
        started.elapsed()
    );
}

#[test]
fn stdio_providers_check_all_provider_smoke_is_batched_not_sequential() {
    let env = fixture_env();
    write_fake_provider(
        &env,
        &[
            "#!/bin/sh",
            "case \"$*\" in",
            "  *--version*)",
            "  echo fake-provider 1.0.0",
            "  exit 0",
            "    ;;",
            "esac",
            "read init || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{}}}'",
            "read new_session || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"sessionId\":\"test-session\"}}'",
            "read prompt_request || exit 1",
            "sleep 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"test-session\",\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"AGENT_BRIDGE_PROVIDER_SMOKE_OK\"}}}}'",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"stopReason\":\"end_turn\"}}'",
            "exit 0",
            "",
        ],
    );
    let mut client = McpClient::start(&env);
    let started = std::time::Instant::now();

    let checks = client.tool(
        "doctor",
        json!({"focus": "providers",
            "smoke": true,
            "aggregateTimeoutMs": 5000,
            "providerTimeoutMs": {
                "claude": 3000,
                "cursor": 3000,
                "kimi": 3000,
                "codex": 3000,
                "forge": 3000,
                "antigravity": 3000
            }
        }),
    );

    assert_eq!(
        sorted_provider_keys(&checks),
        vec!["antigravity", "claude", "codex", "cursor", "forge", "kimi"]
    );
    for provider in ["claude", "cursor", "kimi", "codex", "forge", "antigravity"] {
        assert_eq!(checks["providers"][provider]["startupVerified"], true);
    }
    assert!(
        started.elapsed() < std::time::Duration::from_millis(4600),
        "all-provider smoke should be batched below sequential elapsed time: {:?}",
        started.elapsed()
    );
}

#[test]
fn stdio_providers_check_timeout_fallback_and_process_group_cleanup() {
    let env = fixture_env();
    let child_pid_path = env.log_dir.join("smoke-child.pid");
    write_fake_provider(
        &env,
        &[
            "#!/bin/sh",
            "case \"$*\" in",
            "  *--version*)",
            "  echo fake-provider 1.0.0",
            "  exit 0",
            "    ;;",
            "esac",
            "read init || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{}}}'",
            "read new_session || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"sessionId\":\"test-session\"}}'",
            "read prompt_request || exit 1",
            "sleep 5 &",
            "child=$!",
            "printf '%s\\n' \"$child\" > \"$AGENT_BRIDGE_STATE_DIR/provider-log/smoke-child.pid\"",
            "wait \"$child\"",
            "exit 0",
            "",
        ],
    );
    let mut client = McpClient::start(&env);

    let checks = client.tool(
        "doctor",
        json!({"focus": "providers", "smoke": true, "providers": ["cursor"], "timeoutMs": 800}),
    );
    let cursor = &checks["providers"]["cursor"];
    assert_eq!(cursor["startupVerified"], false);
    assert_eq!(cursor["launchable"], false);
    assert_eq!(cursor["readiness"]["state"], "failed");
    assert_eq!(cursor["readiness"]["launchable"], false);
    assert!(cursor["readiness"]["checkedAt"].is_string());
    assert_eq!(cursor["diagnostic"]["failureCategory"], "provider_timeout");
    assert_eq!(
        cursor["readiness"]["diagnostic"]["failureCategory"],
        "provider_timeout"
    );
    assert_eq!(cursor["diagnostic"]["timeoutMs"], 800);

    let child_pid = std::fs::read_to_string(&child_pid_path)
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();
    for _ in 0..20 {
        if !process_is_alive(child_pid) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("timed out waiting for smoke child pid {child_pid} to be reaped");
}

#[test]
fn stdio_providers_check_aggregate_timeout_kills_inflight_smokes() {
    let env = fixture_env();
    write_fake_provider(
        &env,
        &[
            "#!/bin/sh",
            "stdin=$(cat)",
            "if [ \"$1\" = \"--version\" ]; then",
            "  echo fake-provider 1.0.0",
            "  exit 0",
            "fi",
            "sleep 5 &",
            "child=$!",
            "trap 'kill -TERM \"$child\" 2>/dev/null || true; wait \"$child\" 2>/dev/null || true; exit 143' TERM INT",
            "wait \"$child\"",
            "",
        ],
    );
    let mut client = McpClient::start(&env);

    let checks = client.tool(
        "doctor",
        json!({"focus": "providers",
            "smoke": true,
            "providers": ["cursor", "kimi"],
            "aggregateTimeoutMs": 300,
            "providerTimeoutMs": {"cursor": 3000, "kimi": 3000}
        }),
    );
    assert_eq!(
        checks["providers"]["cursor"]["diagnostic"]["failureCategory"],
        "provider_timeout"
    );
    assert_eq!(
        checks["providers"]["kimi"]["diagnostic"]["failureCategory"],
        "provider_timeout"
    );
}

#[test]
fn stdio_providers_check_concurrency_env_fallbacks() {
    let env = fixture_env();
    write_fake_provider(
        &env,
        &[
            "#!/bin/sh",
            "case \"$*\" in",
            "  *--version*)",
            "  echo fake-provider 1.0.0",
            "  exit 0",
            "    ;;",
            "esac",
            "read init || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{}}}'",
            "read new_session || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"sessionId\":\"test-session\"}}'",
            "read prompt_request || exit 1",
            "sleep 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"test-session\",\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"AGENT_BRIDGE_PROVIDER_SMOKE_OK\"}}}}'",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"stopReason\":\"end_turn\"}}'",
            "exit 0",
            "",
        ],
    );
    let mut extra_env = BTreeMap::new();
    extra_env.insert(
        "AGENT_BRIDGE_SMOKE_CONCURRENCY".to_string(),
        OsString::from("1"),
    );
    let mut client = McpClient::start_with_extra_env(&env, extra_env);
    let started = std::time::Instant::now();
    let checks = client.tool(
        "doctor",
        json!({"focus": "providers",
            "smoke": true,
            "providers": ["cursor", "kimi"],
            "aggregateTimeoutMs": 3000,
            "providerTimeoutMs": {"cursor": 1500, "kimi": 1500}
        }),
    );
    assert_eq!(checks["providers"]["cursor"]["startupVerified"], true);
    assert_eq!(checks["providers"]["kimi"]["startupVerified"], true);
    assert!(
        started.elapsed() >= std::time::Duration::from_millis(1900),
        "concurrency=1 should run probes sequentially: {:?}",
        started.elapsed()
    );
    drop(client);

    let mut extra_env = BTreeMap::new();
    extra_env.insert(
        "AGENT_BRIDGE_SMOKE_CONCURRENCY".to_string(),
        OsString::from("not-a-number"),
    );
    let mut client = McpClient::start_with_extra_env(&env, extra_env);
    let started = std::time::Instant::now();
    let checks = client.tool(
        "doctor",
        json!({"focus": "providers",
            "smoke": true,
            "providers": ["cursor", "kimi"],
            "aggregateTimeoutMs": 2500,
            "providerTimeoutMs": {"cursor": 1500, "kimi": 1500}
        }),
    );
    assert_eq!(checks["providers"]["cursor"]["startupVerified"], true);
    assert_eq!(checks["providers"]["kimi"]["startupVerified"], true);
    assert!(
        started.elapsed() < std::time::Duration::from_millis(1800),
        "invalid concurrency should fall back to default concurrency: {:?}",
        started.elapsed()
    );
}

#[test]
fn stdio_providers_check_hung_version_probe_times_out() {
    let env = fixture_env();
    write_fake_provider(
        &env,
        &[
            "#!/bin/sh",
            "if [ \"$1\" = \"--version\" ]; then",
            "  sleep 5 &",
            "  child=$!",
            "  wait \"$child\"",
            "  exit 0",
            "fi",
            "exit 0",
            "",
        ],
    );
    let mut client = McpClient::start(&env);

    let checks = client.tool(
        "doctor",
        json!({"focus": "providers", "providers": ["codex"]}),
    );
    let codex = &checks["providers"]["codex"];
    assert_eq!(codex["available"], false);
    assert_eq!(codex["diagnostic"]["failureCategory"], "provider_timeout");
    assert_eq!(codex["diagnostic"]["timeoutMs"], 5000);
}

#[test]
fn stdio_workspace_path_list_allows_multiple_roots_and_rejects_outside() {
    let env = fixture_env();
    let second_root = temp_dir("agent-bridge-second-root");
    let workspaces = std::env::join_paths([env.root.as_os_str(), second_root.as_os_str()]).unwrap();
    let mut client = McpClient::start_with_workspace_value(&env, workspaces);

    let preview = client.tool(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "x",
            "cwd": second_root
        , "dryRun": true}),
    );
    assert_eq!(
        preview["cwd"].as_str().unwrap(),
        second_root.canonicalize().unwrap().to_str().unwrap()
    );

    let outside = temp_dir("agent-bridge-outside");
    let error = client.tool_error(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "x",
            "cwd": outside
        , "dryRun": true}),
    );
    assert!(error.contains("outside configured workspaces"));
}

#[test]
fn stdio_ignores_legacy_allowed_root_env_var() {
    let env = fixture_env();
    let mut client = McpClient::start_with_legacy_allowed_root_only(&env);

    let error = client.tool_error(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "x",
            "cwd": env.root
        , "dryRun": true}),
    );
    assert!(error.contains("outside configured workspaces"));
}

#[test]
fn stdio_agent_lifecycle_stop_timeout_and_logs() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let spawned = client.tool(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "emit-logs",
            "cwd": env.root
        }),
    );
    assert_no_task_id_key(&spawned);
    let agent_id = spawned["agentId"].as_str().unwrap();
    assert!(agent_id.starts_with("agent_"));

    let waited = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": agent_id, "timeoutMs": 30000}),
    );
    assert_no_task_id_key(&waited);
    assert_eq!(waited["status"], "succeeded");

    let logs = client.tool(
        "agent_result",
        json!({"agentId": agent_id, "sections": ["stdout", "stderr"]}),
    );
    assert_no_task_id_key(&logs);
    assert!(
        logs["stdout"]
            .as_str()
            .unwrap()
            .contains("lifecycle-stdout")
    );
    assert!(
        logs["stderr"]
            .as_str()
            .unwrap()
            .contains("lifecycle-stderr")
    );

    let result = client.tool("agent_result", json!({"agentId": agent_id}));
    assert_no_task_id_key(&result);
    assert_eq!(result["status"], "succeeded");
    assert_eq!(result["exitCode"], 0);
    assert_eq!(result["reviewPacket"]["status"], "succeeded");
    assert_eq!(result["reviewPacket"]["isFinal"], true);
    assert_eq!(result["reviewPacket"]["hasChanges"], false);
    assert_eq!(result["reviewPacket"]["changedFiles"], json!([]));
    assert_eq!(result["reviewPacket"]["stdoutTruncated"], false);
    assert_eq!(result["reviewPacket"]["stderrTruncated"], false);
    let actions = review_actions_text(&result);
    assert!(
        actions
            .contains("Inspect stdout, stderr, diagnostics, git status, diff, and changed files.")
    );
    assert!(actions.contains("Run the relevant project verification before claiming completion."));

    let active = client.tool(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "sleep-long",
            "cwd": env.root,
            "timeoutSeconds": 20
        }),
    );
    assert_no_task_id_key(&active);
    let active_id = active["agentId"].as_str().unwrap();
    let legacy_id_error = client.tool_error("agent_observe", json!({"taskId": active_id}));
    assert!(legacy_id_error.contains("Unknown argument for agent_observe: taskId"));
    let active_result = client.tool("agent_result", json!({"agentId": active_id}));
    assert_no_task_id_key(&active_result);
    assert_eq!(active_result["reviewPacket"]["isFinal"], false);
    let actions = review_actions_text(&active_result);
    assert!(
        actions.contains(
            "Use agent_observe with a bounded timeout before treating silence as a stall."
        )
    );
    assert!(actions.contains("Use agent_observe with until:final when only finality matters."));
    assert!(
        actions.contains(
            "Use agent_observe with limit:0 to confirm whether the agent is still active."
        )
    );
    assert!(actions.contains("Use agent_stop if the agent is no longer useful."));

    let remove_error = client.tool_error("agent_remove", json!({"agentId": active_id}));
    assert!(remove_error.contains("cannot remove a running agent"));

    let stopped = client.tool("agent_stop", json!({"agentId": active_id}));
    assert_no_task_id_key(&stopped);
    assert_eq!(stopped["status"], "stopped");

    let timed = client.tool(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "sleep-long",
            "cwd": env.root,
            "timeoutSeconds": 1
        }),
    );
    let timed_id = timed["agentId"].as_str().unwrap();
    let timed_wait = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": timed_id, "timeoutMs": 30000}),
    );
    assert_eq!(timed_wait["isFinal"], true);
    if timed_wait["status"] == "failed" {
        assert_eq!(timed_wait["errorType"], "timeout");
    }
}

#[test]
fn stdio_loads_legacy_registry_records_without_public_task_id_alias() {
    let env = fixture_env();
    let legacy_id = "task_legacyregistry000000000000000001";
    let legacy_dir = env.state_dir.join("tasks").join(legacy_id);
    std::fs::create_dir_all(&legacy_dir).unwrap();
    std::fs::write(legacy_dir.join("stdout.log"), "legacy stdout\n").unwrap();
    std::fs::write(legacy_dir.join("stderr.log"), "legacy stderr\n").unwrap();
    std::fs::write(
        env.state_dir.join("registry.json"),
        serde_json::to_vec_pretty(&json!({
            "tasks": {
                legacy_id: {
                    "taskId": legacy_id,
                    "provider": "codex",
                    "mode": "review",
                    "title": "Legacy registry record",
                    "status": "succeeded",
                    "cwd": env.root,
                    "isolation": "none",
                    "taskDir": legacy_dir,
                    "command": "codex",
                    "args": [],
                    "timeoutSeconds": 1,
                    "profile": "bridge",
                    "promptStrategy": "bridge",
                    "createdAt": "2026-06-01T00:00:00.000Z",
                    "updatedAt": "2026-06-01T00:01:00.000Z",
                    "startedAt": "2026-06-01T00:00:00.000Z",
                    "completedAt": "2026-06-01T00:01:00.000Z",
                    "exitCode": 0
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();
    let mut client = McpClient::start(&env);

    let doctor = client.tool("doctor", json!({"cwd": env.root}));
    assert_eq!(doctor["state"]["status"], "ok");

    let listed = client.tool("agent_list", json!({}));
    assert_no_task_id_key(&listed);
    assert_eq!(listed["agents"].as_array().unwrap().len(), 1);
    assert_eq!(listed["agents"][0]["agentId"], legacy_id);
    assert_eq!(listed["agents"][0]["status"], "succeeded");

    let legacy_input_error = client.tool_error("agent_observe", json!({"taskId": legacy_id}));
    assert!(legacy_input_error.contains("Unknown argument for agent_observe: taskId"));

    let result = client.tool(
        "agent_result",
        json!({"agentId": legacy_id, "sections": ["stdout", "stderr"]}),
    );
    assert_no_task_id_key(&result);
    assert_eq!(result["agentId"], legacy_id);
    assert!(result["stdout"].as_str().unwrap().contains("legacy stdout"));
    assert!(result["stderr"].as_str().unwrap().contains("legacy stderr"));
    assert_eq!(result["reviewPacket"]["isFinal"], true);
}

#[test]
fn stdio_agent_observe_returns_events_and_progress() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let spawned = client.tool(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "emit-logs",
            "cwd": env.root,
            "timeoutSeconds": 5
        }),
    );
    let agent_id = spawned["agentId"].as_str().unwrap();

    let observed = client.tool(
        "agent_observe",
        json!({"agentId": agent_id, "cursor": 0, "limit": 100, "timeoutMs": 5000}),
    );
    assert_no_task_id_key(&observed);
    assert_eq!(observed["agentId"], agent_id);
    // Lean envelope: no nested `agent` echo or `task` object.
    assert!(observed.get("agent").is_none());
    assert!(observed.get("task").is_none());
    assert!(observed.get("presentation").is_none());
    assert!(observed["next"].is_array());
    assert!(!observed["events"].as_array().unwrap().is_empty());
    assert!(observed["nextCursor"].as_u64().unwrap() >= 1);
    assert!(observed["progress"]["elapsedMs"].is_number());
    assert!(observed["progress"]["expectedOutputCadence"].is_object());
    assert_eq!(
        observed["next"][0]["tool"],
        if observed["isFinal"].as_bool().unwrap() {
            "agent_result"
        } else {
            "agent_observe"
        }
    );
}

#[test]
fn stdio_agent_observe_timeout_does_not_fail_running_agent() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let spawned = client.tool(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "sleep-long",
            "cwd": env.root,
            "timeoutSeconds": 20
        }),
    );
    let agent_id = spawned["agentId"].as_str().unwrap();
    std::thread::sleep(Duration::from_millis(250));
    let current = client.tool(
        "agent_result",
        json!({"agentId": agent_id, "cursor": 0, "limit": 100, "sections": ["transcript"]}),
    )["transcript"]
        .clone();
    assert_no_task_id_key(&current);
    let cursor = current["nextCursor"].as_u64().unwrap();
    let second = client.tool(
        "agent_observe",
        json!({"agentId": agent_id, "cursor": cursor, "limit": 100, "timeoutMs": 100}),
    );
    assert_no_task_id_key(&second);

    assert!(
        second["timedOut"].as_bool().unwrap() || !second["events"].as_array().unwrap().is_empty()
    );
    assert_eq!(second["status"], "running");
    assert_eq!(second["isFinal"], false);
    assert_eq!(second["progress"]["noFurtherPollingNeeded"], false);

    let stopped = client.tool("agent_stop", json!({"agentId": agent_id}));
    assert_eq!(stopped["status"], "stopped");
}

#[test]
fn stdio_agent_list_defaults_to_native_presentation_and_filters() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let completed = client.tool(
        "agent_spawn",
        json!({
            "provider": "cursor",
            "mode": "review",
            "title": "Native UX review",
            "prompt": "emit-logs",
            "cwd": env.root,
            "timeoutSeconds": 5
        }),
    );
    let completed_id = completed["agentId"].as_str().unwrap().to_string();
    assert!(completed["next"].is_array());
    let waited = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": completed_id, "timeoutMs": 30000}),
    );
    assert_eq!(waited["status"], "succeeded");

    let active = client.tool(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "title": "Active native UX task",
            "prompt": "sleep-long",
            "cwd": env.root,
            "timeoutSeconds": 20
        }),
    );
    let active_id = active["agentId"].as_str().unwrap().to_string();

    let listed = client.tool("agent_list", json!({}));
    assert_no_task_id_key(&listed);
    assert_eq!(listed["scope"], "active_recent");
    assert_eq!(listed["limit"], 25);
    assert!(listed.get("tasks").is_none());
    let agents = listed["agents"].as_array().unwrap();
    assert_eq!(agents.len(), 2);
    // Lean per-agent summaries: a single `next` list, no GUI presentation/actions blob.
    assert_eq!(agents[0]["agentId"], active_id);
    assert_eq!(agents[0]["phase"], "active");
    assert!(agents[0].get("presentation").is_none());
    assert_eq!(agents[0]["next"][0]["id"], "observe");
    assert_eq!(agents[0]["next"][0]["tool"], "agent_observe");
    assert_eq!(agents[1]["agentId"], completed_id);
    assert_eq!(agents[1]["status"], "succeeded");
    assert_eq!(agents[1]["isFinal"], true);
    assert!(agents[1]["next"].is_array());
    assert!(agents[1].get("presentation").is_none());
    assert!(agents[1].get("stdout").is_none());
    assert!(agents[1].get("gitDiff").is_none());

    let filtered = client.tool(
        "agent_list",
        json!({
            "provider": ["cursor"],
            "mode": ["review"],
            "cwd": env.root,
            "titleContains": "ux",
            "limit": 1
        }),
    );
    assert_no_task_id_key(&filtered);
    assert_eq!(filtered["agents"].as_array().unwrap().len(), 1);
    assert_eq!(filtered["agents"][0]["agentId"], completed_id);

    let filtered_agents = client.tool(
        "agent_list",
        json!({
            "provider": ["cursor"],
            "mode": ["review"],
            "cwd": env.root,
            "titleContains": "ux",
            "limit": 1
        }),
    );
    assert_eq!(filtered_agents["agents"].as_array().unwrap().len(), 1);
    assert_eq!(filtered_agents["agents"][0]["agentId"], completed_id);

    let error = client.tool_error("agent_list", json!({"limit": 101}));
    assert!(error.contains("limit must be between 1 and 100"));
    let error = client.tool_error("agent_list", json!({"presentation": false}));
    assert!(error.contains("Unknown argument for agent_list: presentation"));
    let error = client.tool_error("agent_list", json!({"scope": "all"}));
    assert!(error.contains("Unknown argument for agent_list: scope"));

    let stopped = client.tool("agent_stop", json!({"agentId": active_id}));
    assert_eq!(stopped["status"], "stopped");
}

#[test]
fn stdio_sends_agent_completion_notification_with_compact_summary() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let spawned = client.tool(
        "agent_spawn",
        json!({
            "provider": "cursor",
            "mode": "review",
            "title": "Native UX review",
            "prompt": "emit-logs",
            "cwd": env.root,
            "timeoutSeconds": 5
        }),
    );
    let agent_id = spawned["agentId"].as_str().unwrap().to_string();

    let notification = client.notification("notifications/agent_bridge/agent_completed");
    assert!(notification.get("id").is_none());
    assert_eq!(
        notification["method"],
        "notifications/agent_bridge/agent_completed"
    );
    let params = &notification["params"];
    assert_eq!(params["agentId"], agent_id);
    assert_eq!(params["displayTitle"], "Native UX review");
    assert_eq!(params["status"], "succeeded");
    assert_eq!(params["isFinal"], true);
    assert_eq!(params["attentionRequired"], true);
    assert_eq!(params["summary"]["exitCode"], 0);
    assert!(params["summary"]["changedFileCount"].is_number());
    assert_eq!(params["summary"]["next"][0]["id"], "inspect_result");
    assert_eq!(params["summary"]["next"][0]["tool"], "agent_result");
    assert!(params.get("stdout").is_none());
    assert!(params.get("stderr").is_none());
    assert!(params.get("gitDiff").is_none());
    assert!(params.get("transcript").is_none());
    assert!(params["summary"].get("partialResults").is_none());

    let result = client.tool("agent_result", json!({"agentId": agent_id}));
    assert_eq!(result["agentId"], agent_id);
    assert_eq!(result["status"], "succeeded");
    assert_eq!(result["reviewPacket"]["agentId"], agent_id);

    let listed = client.tool("agent_list", json!({}));
    assert!(
        listed["agents"]
            .as_array()
            .unwrap()
            .iter()
            .all(|agent| agent["agentId"] != agent_id)
    );

    let filtered = client.tool(
        "agent_list",
        json!({
            "provider": ["cursor"],
            "mode": ["review"],
            "titleContains": "ux"
        }),
    );
    assert_eq!(
        filtered["agents"].as_array().unwrap()[0]["agentId"],
        agent_id
    );
}

#[test]
fn stdio_sends_completion_notification_for_launch_failure() {
    let env = fixture_env();
    let mut extra_env = BTreeMap::new();
    extra_env.insert(
        "CURSOR_AGENT_BIN".to_string(),
        env.root.join("missing-provider").into_os_string(),
    );
    extra_env.insert(
        "CURSOR_ACP_BIN".to_string(),
        env.root.join("missing-provider").into_os_string(),
    );
    let mut client = McpClient::start_with_extra_env(&env, extra_env);

    let spawned = client.tool(
        "agent_spawn",
        json!({
            "provider": "cursor",
            "mode": "review",
            "title": "Broken launch",
            "prompt": "emit-logs",
            "cwd": env.root,
            "timeoutSeconds": 5
        }),
    );
    let agent_id = spawned["agentId"].as_str().unwrap().to_string();
    assert_eq!(spawned["status"], "failed");
    assert_eq!(spawned["isFinal"], true);

    let notification = client.notification("notifications/agent_bridge/agent_completed");
    let params = &notification["params"];
    assert_eq!(params["agentId"], agent_id);
    assert_eq!(params["status"], "failed");
    assert_eq!(params["summary"]["errorType"], "provider_start_error");
    assert_eq!(params["summary"]["next"][0]["id"], "inspect_result");
}

#[test]
fn stdio_claude_agent_runs_prompt_through_acp_stdin() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);
    let prompt = "--leading-flag\nquoted \"value\" $(touch should-not-run) secret-token";

    let spawned = client.tool(
        "agent_spawn",
        json!({
            "provider": "claude",
            "mode": "review",
            "prompt": prompt,
            "cwd": env.root
        }),
    );
    let agent_id = spawned["agentId"].as_str().unwrap();
    let waited = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": agent_id, "timeoutMs": 30000}),
    );
    assert_eq!(waited["status"], "succeeded");

    let result = client.tool("agent_result", json!({"agentId": agent_id}));
    assert_eq!(result["status"], "succeeded");
    assert!(result.get("stdout").is_none());
    assert!(result.get("stderr").is_none());
    assert!(result.get("gitDiff").is_none());
    assert!(result.get("transcript").is_none());

    let raw = client.tool(
        "agent_result",
        json!({"agentId": agent_id, "sections": ["stdout", "stderr", "transcript"]}),
    );
    assert!(raw.get("stdout").is_some());
    assert!(raw.get("stderr").is_some());
    assert!(raw.get("transcript").is_some());

    assert!(!env.root.join("should-not-run").exists());
    let stdin = std::fs::read_to_string(env.log_dir.join("stdin.txt")).unwrap();
    assert!(stdin.contains("Return only the task-relevant final answer"));
    assert!(stdin.contains("Do not echo source text"));
    assert!(stdin.contains("narrate progress/polling/waiting"));
    assert!(stdin.contains("include generic checklists"));
    assert!(stdin.contains("omit empty sections"));
    assert!(!stdin.contains("Return a concise final report"));
    assert!(!stdin.contains("summary, changed files if any, evidence, risks, and next steps"));
}

#[test]
fn stdio_claude_no_host_runner_ignores_zsh_startup_files() {
    let env = fixture_env();
    let home = temp_dir("agent-bridge-home");
    std::fs::write(home.join(".zshenv"), "cat >/dev/null || true\n").unwrap();
    let mut extra_env = BTreeMap::new();
    extra_env.insert("HOME".to_string(), home.into_os_string());
    let mut client = McpClient::start_with_extra_env(&env, extra_env);

    let spawned = client.tool(
        "agent_spawn",
        json!({
            "provider": "claude",
            "mode": "review",
            "prompt": "terminal-noise",
            "cwd": env.root,
            "timeoutSeconds": 5
        }),
    );
    let agent_id = spawned["agentId"].as_str().unwrap();

    let waited = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": agent_id, "timeoutMs": 30000}),
    );
    assert_eq!(waited["status"], "succeeded");

    let result = client.tool("agent_result", json!({"agentId": agent_id}));
    assert_eq!(result["status"], "succeeded");
}

#[test]
fn stdio_claude_preview_uses_acp_transport() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let preview = client.tool(
        "agent_spawn",
        json!({
            "provider": "claude",
            "mode": "review",
            "prompt": "native prompt",
            "cwd": env.root
        , "dryRun": true}),
    );
    let args = preview["args"].as_array().unwrap();
    assert_eq!(
        preview["command"].as_str().unwrap(),
        env.fake_provider.to_string_lossy()
    );
    assert!(args.is_empty());
    assert_eq!(preview["stdin"], "<prompt redacted>");
    assert_eq!(preview["commandKind"], "acp");
    assert_eq!(preview["launchStrategy"], "acp");
}

#[test]
fn stdio_claude_smoke_timeout_returns_bounded_diagnostic() {
    let env = fixture_env();
    std::fs::write(
        &env.fake_provider,
        [
            "#!/bin/sh",
            "case \"$*\" in",
            "  *--version*)",
            "  echo fake-provider 1.0.0",
            "  exit 0",
            "    ;;",
            "esac",
            "read init || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{}}}'",
            "read new_session || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"sessionId\":\"test-session\"}}'",
            "read prompt_request || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"test-session\",\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"owned claude runner booting\"}}}}'",
            "echo waiting for stop hook >&2",
            "sleep 2 &",
            "child=$!",
            "trap 'kill -TERM \"$child\" 2>/dev/null || true; wait \"$child\" 2>/dev/null || true; exit 143' TERM INT",
            "wait \"$child\"",
            "",
        ]
        .join("\n"),
    )
    .unwrap();
    make_executable(&env.fake_provider);
    let mut client = McpClient::start(&env);

    let checks = client.tool(
        "doctor",
        json!({"focus": "providers", "smoke": true, "timeoutMs": 500}),
    );
    let claude = &checks["providers"]["claude"];
    assert_eq!(claude["available"], false);
    assert_eq!(claude["startupVerified"], false);
    assert_eq!(claude["diagnostic"]["failureCategory"], "provider_timeout");
    assert_eq!(claude["diagnostic"]["timeoutMs"], 500);
    assert_eq!(claude["diagnostic"]["launchStrategy"], "acp");
}

#[test]
fn stdio_claude_agent_malformed_output_returns_diagnostic() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let spawned = client.tool(
        "agent_spawn",
        json!({
            "provider": "claude",
            "mode": "review",
            "prompt": "malformed-output",
            "cwd": env.root
        }),
    );
    let agent_id = spawned["agentId"].as_str().unwrap();
    let waited = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": agent_id, "timeoutMs": 30000}),
    );
    assert_eq!(waited["status"], "failed");
    assert_eq!(waited["errorType"], "provider_output_error");

    let result = client.tool("agent_result", json!({"agentId": agent_id}));
    assert_eq!(result["errorType"], "provider_output_error");
    assert_eq!(result["reviewPacket"]["status"], "failed");
    assert_eq!(result["reviewPacket"]["errorType"], "provider_output_error");
    assert_eq!(result["reviewPacket"]["exitCode"], Value::Null);
    let actions = review_actions_text(&result);
    assert!(
        actions.contains("Inspect logs and diagnostic metadata before deciding whether to rerun.")
    );
    assert!(actions.contains(
        "Decide whether to rerun with a narrower prompt, continue manually, or discard."
    ));
}

#[test]
fn stdio_claude_agent_failure_modes_are_classified() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let cases = [
        ("non-zero-exit", "provider_output_error", 5),
        ("missing-result", "provider_output_error", 5),
        ("claude-timeout", "timeout", 1),
    ];
    for (prompt, error_type, timeout_seconds) in cases {
        let spawned = client.tool(
            "agent_spawn",
            json!({
                "provider": "claude",
                "mode": "review",
                "prompt": prompt,
                "cwd": env.root,
                "timeoutSeconds": timeout_seconds
            }),
        );
        let agent_id = spawned["agentId"].as_str().unwrap();
        let waited = client.tool(
            "agent_observe",
            json!({"until": "final", "verbosity": "detailed", "agentId": agent_id, "timeoutMs": 30000}),
        );
        assert_eq!(waited["status"], "failed", "{prompt}");
        assert_eq!(waited["errorType"], error_type, "{prompt}");

        let result = client.tool("agent_result", json!({"agentId": agent_id}));
        assert_eq!(result["errorType"], error_type, "{prompt}");
    }
}

#[test]
fn stdio_claude_stop_reason_is_structured_diagnostic() {
    let env = fixture_env();
    write_fake_provider(
        &env,
        &[
            "#!/bin/sh",
            "case \"$*\" in",
            "  *--version*) echo fake-provider 1.0.0; exit 0 ;;",
            "esac",
            "read init || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{}}}'",
            "read new_session || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"sessionId\":\"test-session\"}}'",
            "read prompt_request || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"test-session\",\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"cannot comply\"}}}}'",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"stopReason\":\"refusal\"}}'",
        ],
    );
    let mut client = McpClient::start(&env);

    let spawned = client.tool(
        "agent_spawn",
        json!({
            "provider": "claude",
            "mode": "review",
            "prompt": "refusal",
            "cwd": env.root,
            "timeoutSeconds": 5
        }),
    );
    let agent_id = spawned["agentId"].as_str().unwrap();
    let waited = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": agent_id, "timeoutMs": 30000}),
    );
    assert_eq!(waited["status"], "failed");
    assert_eq!(waited["errorType"], "provider_output_error");

    let result = client.tool("agent_result", json!({"agentId": agent_id}));
    assert_eq!(
        result["reviewPacket"]["diagnostic"]["acpStopReason"],
        "refusal"
    );
}

#[test]
fn stdio_claude_agent_extracts_result_with_surrounding_noise() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let spawned = client.tool(
        "agent_spawn",
        json!({
            "provider": "claude",
            "mode": "review",
            "prompt": "terminal-noise",
            "cwd": env.root
        }),
    );
    let agent_id = spawned["agentId"].as_str().unwrap();
    let waited = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": agent_id, "timeoutMs": 30000}),
    );
    assert_eq!(waited["status"], "succeeded");
}

#[test]
fn stdio_claude_agent_diagnostic_redacts_prompt_content() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let spawned = client.tool(
        "agent_spawn",
        json!({
            "provider": "claude",
            "mode": "review",
            "prompt": "malformed-output secret-token-for-redaction",
            "cwd": env.root
        }),
    );
    let agent_id = spawned["agentId"].as_str().unwrap();
    let waited = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": agent_id, "timeoutMs": 30000}),
    );
    assert_eq!(waited["status"], "failed");

    let result = client.tool("agent_result", json!({"agentId": agent_id}));
    let diagnostic = serde_json::to_string(&result["reviewPacket"]["diagnostic"]).unwrap();
    assert!(
        !diagnostic.contains("secret-token-for-redaction"),
        "diagnostic leaked prompt content: {diagnostic}"
    );
}

#[test]
fn stdio_claude_agent_diagnostic_redacts_token_values() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let spawned = client.tool(
        "agent_spawn",
        json!({
            "provider": "claude",
            "mode": "review",
            "prompt": "echo-api-key-fail",
            "cwd": env.root
        }),
    );
    let agent_id = spawned["agentId"].as_str().unwrap();
    let waited = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": agent_id, "timeoutMs": 30000}),
    );
    assert_eq!(waited["status"], "failed");

    let result = client.tool("agent_result", json!({"agentId": agent_id}));
    let diagnostic = serde_json::to_string(&result["reviewPacket"]["diagnostic"]).unwrap();
    assert!(!diagnostic.contains("test-key"), "diagnostic leaked token");
}

#[test]
fn stdio_agent_result_review_packet_summarizes_worktree_changes() {
    let env = fixture_env();
    write_fake_provider(
        &env,
        &[
            "#!/bin/sh",
            "case \"$*\" in",
            "  *--version*)",
            "  echo fake-provider 1.0.0",
            "  exit 0",
            "    ;;",
            "esac",
            "read init || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{}}}'",
            "read new_session || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"sessionId\":\"test-session\"}}'",
            "read prompt_request || exit 1",
            "case \"$prompt_request\" in",
            "  *modify-readme*) printf 'changed by provider\\n' >> README.md ;;",
            "esac",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"test-session\",\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"modified-readme\"}}}}'",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"stopReason\":\"end_turn\"}}'",
            "exit 0",
            "",
        ],
    );
    let repo = env.root.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    init_git_repo(&repo);

    let mut client = McpClient::start(&env);
    let spawned = client.tool(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "implement",
            "prompt": "modify-readme",
            "cwd": repo,
            "isolation": "worktree",
            "worktreeName": "review-packet"
        }),
    );
    let agent_id = spawned["agentId"].as_str().unwrap();
    let waited = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": agent_id, "timeoutMs": 30000}),
    );
    assert_eq!(waited["status"], "succeeded");

    let result = client.tool("agent_result", json!({"agentId": agent_id}));
    assert_eq!(result["changedFiles"], json!(["README.md"]));
    assert_eq!(result["reviewPacket"]["hasChanges"], true);
    assert_eq!(result["reviewPacket"]["changedFiles"], json!(["README.md"]));
    assert!(
        result["reviewPacket"]["gitStatusSummary"]
            .as_str()
            .unwrap()
            .contains("README.md")
    );
    let actions = review_actions_text(&result);
    assert!(actions.contains("Inspect gitStatus, gitDiff, and changedFiles before verification."));
    assert!(
        actions.contains("Call agent_remove only after inspecting the managed worktree result.")
    );
}

#[test]
fn stdio_managed_worktree_lifecycle() {
    let env = fixture_env();
    let repo = env.root.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    init_git_repo(&repo);

    let mut client = McpClient::start(&env);
    let spawned = client.tool(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "emit-logs",
            "cwd": repo,
            "isolation": "worktree",
            "worktreeName": "fixture-rust"
        }),
    );
    assert_eq!(spawned["isolation"], "worktree");
    let agent_id = spawned["agentId"].as_str().unwrap();
    let worktree_path = PathBuf::from(spawned["worktreePath"].as_str().unwrap());
    assert!(worktree_path.exists());

    let waited = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": agent_id, "timeoutMs": 30000}),
    );
    assert_eq!(waited["status"], "succeeded");
    // Lean envelope: a single `next` list (no GUI presentation/actions).
    assert_eq!(waited["next"][0]["id"], "inspect_result");
    assert_eq!(waited["next"][0]["arguments"]["agentId"], agent_id);
    let cleanup_before_result = waited["next"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["id"] == "cleanup")
        .unwrap();
    assert_eq!(cleanup_before_result["state"], "unsafe");
    assert_eq!(cleanup_before_result["safety"], "unsafe");

    let result = client.tool("agent_result", json!({"agentId": agent_id}));
    assert_eq!(result["reviewPacket"]["gitStatusSummary"], "");
    assert_eq!(result["changedFiles"], json!([]));
    assert_eq!(result["next"][0]["id"], "verify_project");
    let cleanup_after_result = result["next"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["id"] == "cleanup")
        .unwrap();
    assert_eq!(cleanup_after_result["safety"], "destructive");
    assert_eq!(cleanup_after_result["tool"], "agent_remove");
    assert_eq!(cleanup_after_result["state"], "available");

    let removed = client.tool("agent_remove", json!({"agentId": agent_id}));
    assert_eq!(removed["status"], "removed");
    assert!(!worktree_path.exists());
}

#[test]
fn stdio_codex_agent_sandbox_denial_exits_immediately() {
    let env = fixture_env();
    write_fake_provider(
        &env,
        &[
            "#!/bin/sh",
            "case \"$*\" in",
            "  *--version*)",
            "  echo fake-provider 1.0.0",
            "  exit 0",
            "    ;;",
            "esac",
            "read init || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{}}}'",
            "read new_session || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"sessionId\":\"test-session\"}}'",
            "read prompt_request || exit 1",
            "echo 'patch rejected: writing outside of the project; rejected by user approval settings' >&2",
            "exit 1",
            "",
        ],
    );
    let mut client = McpClient::start(&env);

    let spawned = client.tool(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "implement",
            "prompt": "codex-sandbox-denial",
            "cwd": env.root
        }),
    );
    let agent_id = spawned["agentId"].as_str().unwrap();
    let waited = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": agent_id, "timeoutMs": 30000}),
    );
    assert_eq!(waited["status"], "failed");
    assert_eq!(waited["errorType"], "codex_sandbox_denied");

    let logs = client.tool(
        "agent_result",
        json!({"agentId": agent_id, "sections": ["stdout", "stderr"]}),
    );
    assert!(logs["stderr"].as_str().unwrap().contains("patch rejected"));

    let result = client.tool("agent_result", json!({"agentId": agent_id}));
    assert_eq!(
        result["reviewPacket"]["diagnostic"]["failureCategory"],
        "provider_sandbox_denied"
    );
    assert_eq!(result["reviewPacket"]["diagnostic"]["provider"], "codex");
    assert_eq!(
        result["reviewPacket"]["diagnostic"]["launchStrategy"],
        "acp"
    );
    assert_eq!(result["reviewPacket"]["status"], "failed");
    assert_eq!(result["reviewPacket"]["errorType"], "codex_sandbox_denied");

    let actions = review_actions_text(&result);
    for expected in ["logs", "cwd", "workspace", "prompt", "isolation"] {
        assert!(
            actions.contains(expected),
            "actions should mention {expected}: {actions}"
        );
    }
    assert!(
        actions.contains("Do not silently relax sandbox permissions"),
        "actions should reject unsafe sandbox relaxation: {actions}"
    );
    assert!(
        actions.contains("blindly retry"),
        "actions should warn against blind retry: {actions}"
    );
}

#[test]
fn stdio_codex_agent_sandbox_denial_hangs_and_is_terminated_early() {
    let env = fixture_env();
    write_fake_provider(
        &env,
        &[
            "#!/bin/sh",
            "case \"$*\" in",
            "  *--version*)",
            "  echo fake-provider 1.0.0",
            "  exit 0",
            "    ;;",
            "esac",
            "read init || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{}}}'",
            "read new_session || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"sessionId\":\"test-session\"}}'",
            "read prompt_request || exit 1",
            "echo 'patch rejected: writing outside of the project; rejected by user approval settings' >&2",
            "sleep 20 &",
            "child=$!",
            "printf '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"test-session\",\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"child=%s\"}}}}\\n' \"$child\"",
            "wait \"$child\"",
            "",
        ],
    );
    let mut client = McpClient::start(&env);

    let spawned = client.tool(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "implement",
            "prompt": "codex-sandbox-denial-hang",
            "cwd": env.root,
            "timeoutSeconds": 20
        }),
    );
    let agent_id = spawned["agentId"].as_str().unwrap();
    let pid = spawned["pid"].as_i64().unwrap() as i32;
    let waited = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": agent_id, "timeoutMs": 30000}),
    );
    assert_eq!(waited["status"], "failed");
    assert_eq!(waited["errorType"], "codex_sandbox_denied");

    let result = client.tool("agent_result", json!({"agentId": agent_id}));
    assert_eq!(
        result["reviewPacket"]["diagnostic"]["failureCategory"],
        "provider_sandbox_denied"
    );
    assert_eq!(result["reviewPacket"]["diagnostic"]["provider"], "codex");
    let logs = client.tool(
        "agent_result",
        json!({"agentId": agent_id, "sections": ["stdout", "stderr"]}),
    );
    let stdout = logs["stdout"].as_str().unwrap();
    let child_pid = stdout
        .lines()
        .find_map(|line| line.strip_prefix("child="))
        .and_then(|pid| pid.parse::<i32>().ok())
        .expect("fake provider should emit child pid");
    for _ in 0..20 {
        if !process_is_alive(child_pid) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        !process_is_alive(pid),
        "provider process should be reaped after fatal denial"
    );
    assert!(
        !process_is_alive(child_pid),
        "provider child process should be terminated after fatal denial"
    );
}

#[test]
fn stdio_codex_agent_sandbox_denial_redacts_prompt_and_secrets() {
    let env = fixture_env();
    write_fake_provider(
        &env,
        &[
            "#!/bin/sh",
            "case \"$*\" in",
            "  *--version*)",
            "  echo fake-provider 1.0.0",
            "  exit 0",
            "    ;;",
            "esac",
            "read init || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{}}}'",
            "read new_session || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"sessionId\":\"test-session\"}}'",
            "read prompt_request || exit 1",
            "echo 'patch rejected: outside of the project' >&2",
            "printf '%s\n' \"$prompt_request\" >&2",
            "echo \"secret: $ANTHROPIC_API_KEY\" >&2",
            "exit 1",
            "",
        ],
    );
    let mut client = McpClient::start(&env);

    let spawned = client.tool(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "implement",
            "prompt": "secret-prompt-content",
            "cwd": env.root
        }),
    );
    let agent_id = spawned["agentId"].as_str().unwrap();
    let waited = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": agent_id, "timeoutMs": 30000}),
    );
    assert_eq!(waited["status"], "failed");

    let result = client.tool("agent_result", json!({"agentId": agent_id}));
    let diagnostic_text = serde_json::to_string(&result["reviewPacket"]["diagnostic"]).unwrap();
    assert!(
        !diagnostic_text.contains("secret-prompt-content"),
        "diagnostic leaked prompt content: {diagnostic_text}"
    );
    assert!(
        !diagnostic_text.contains("test-key"),
        "diagnostic leaked token: {diagnostic_text}"
    );

    let review_packet_text = serde_json::to_string(&result["reviewPacket"]).unwrap();
    assert!(
        !review_packet_text.contains("secret-prompt-content"),
        "reviewPacket leaked prompt content: {review_packet_text}"
    );
    assert!(
        !review_packet_text.contains("test-key"),
        "reviewPacket leaked token: {review_packet_text}"
    );
}

#[test]
fn stdio_codex_agent_success_still_reports_success() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let spawned = client.tool(
        "agent_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "emit-logs",
            "cwd": env.root
        }),
    );
    let agent_id = spawned["agentId"].as_str().unwrap();
    let waited = client.tool(
        "agent_observe",
        json!({"until": "final", "verbosity": "detailed", "agentId": agent_id, "timeoutMs": 30000}),
    );
    assert_eq!(waited["status"], "succeeded");
    assert_eq!(waited["errorType"], Value::Null);

    let result = client.tool("agent_result", json!({"agentId": agent_id}));
    assert_eq!(result["status"], "succeeded");
    assert_eq!(result["errorType"], Value::Null);
    assert_eq!(result["reviewPacket"]["status"], "succeeded");
    assert_eq!(result["reviewPacket"]["isFinal"], true);
    assert_eq!(result["reviewPacket"]["errorType"], Value::Null);
}
