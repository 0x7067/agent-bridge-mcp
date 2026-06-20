use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use uuid::Uuid;

static PROVIDER_READINESS_TEST_LOCK: Mutex<()> = Mutex::new(());

struct McpClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
}

struct FixtureEnv {
    _guard: MutexGuard<'static, ()>,
    root: PathBuf,
    state_dir: PathBuf,
    fake_provider: PathBuf,
    log_dir: PathBuf,
}

impl FixtureEnv {
    fn new(label: &str) -> Self {
        let guard = provider_readiness_test_guard();
        let root = temp_dir(&format!("agent-bridge-root-{label}"));
        let state_dir = temp_dir(&format!("agent-bridge-state-{label}"));
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
                "    printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"test-session\",\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"{\\\"type\\\":\\\"\\\"}}\"}}}}'",
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
}

impl McpClient {
    fn start_with_command(mut command: Command) -> Self {
        let mut child = command.spawn().unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            stdout,
            next_id: 1,
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

        let message = self.read_message();
        assert_eq!(
            message.get("id").and_then(Value::as_i64),
            Some(id),
            "expected MCP response for id={id}, got {message}"
        );
        message
    }

    fn read_message(&mut self) -> Value {
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        assert!(!line.is_empty(), "expected MCP message");
        serde_json::from_str(&line).unwrap()
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
    FixtureEnv::new("default")
}

fn provider_readiness_test_guard() -> MutexGuard<'static, ()> {
    PROVIDER_READINESS_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn provider_keys(value: &Value) -> Vec<String> {
    value["providers"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect()
}

fn acp_router_command(env: &FixtureEnv) -> Command {
    acp_router_command_with_bins(env, &env.fake_provider, &env.fake_provider)
}

fn acp_router_command_with_bins(env: &FixtureEnv, claude_bin: &Path, codex_bin: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_agent-bridge-mcp"));
    command
        .env_remove("AGENT_BRIDGE_ALLOWED_ROOT")
        .env_remove("AGENT_BRIDGE_WORKSPACES")
        .env_remove("AGENT_BRIDGE_STATE_DIR")
        .env("HOME", &env.root)
        .env("AGENT_BRIDGE_WORKSPACES", &env.root)
        .env("AGENT_BRIDGE_STATE_DIR", &env.state_dir)
        .env("CODEX_ACP_BIN", codex_bin)
        .env("CLAUDE_ACP_BIN", claude_bin)
        .env("CODEX_BIN", codex_bin)
        .env("CLAUDE_BIN", claude_bin)
        .env("ANTHROPIC_API_KEY", "test-key")
        .env("ANTHROPIC_AUTH_TOKEN", "test-auth-token")
        .env("CLAUDE_CODE_OAUTH_TOKEN", "test-code-oauth-token")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

fn mcp_adapter_command(env: &FixtureEnv) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_agent-bridge-mcp"));
    command
        .arg("mcp-adapter")
        .env_remove("AGENT_BRIDGE_ALLOWED_ROOT")
        .env_remove("AGENT_BRIDGE_WORKSPACES")
        .env_remove("AGENT_BRIDGE_STATE_DIR")
        .env("HOME", &env.root)
        .env("AGENT_BRIDGE_WORKSPACES", &env.root)
        .env("AGENT_BRIDGE_STATE_DIR", &env.state_dir)
        .env("CODEX_ACP_BIN", &env.fake_provider)
        .env("CLAUDE_ACP_BIN", &env.fake_provider)
        .env("CODEX_BIN", &env.fake_provider)
        .env("CLAUDE_BIN", &env.fake_provider)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
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

fn assert_json_rpc_message(message: &Value) {
    assert_eq!(message["jsonrpc"], "2.0", "{message}");
    assert!(
        message.get("id").is_some() || message.get("method").is_some(),
        "{message}"
    );
}

fn start_acp_router_session(
    env: &FixtureEnv,
) -> (Child, ChildStdin, BufReader<ChildStdout>, String) {
    start_acp_router_session_with_command(env, acp_router_command(env))
}

fn start_acp_router_session_with_command(
    env: &FixtureEnv,
    mut command: Command,
) -> (Child, ChildStdin, BufReader<ChildStdout>, String) {
    let mut child = command
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
    let initialize = read_json_line(&mut stdout);
    assert_json_rpc_message(&initialize);
    assert_eq!(initialize["result"]["protocolVersion"], 1);
    writeln!(
        stdin,
        "{}",
        json!({"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd": env.root}})
    )
    .unwrap();
    let session = read_json_line(&mut stdout);
    assert_json_rpc_message(&session);
    let session_id = session["result"]["sessionId"].as_str().unwrap().to_string();
    (child, stdin, stdout, session_id)
}

fn write_acp_router_claude_review_prompt(
    stdin: &mut ChildStdin,
    session_id: &str,
    prompt_text: &str,
) {
    write_acp_router_review_prompt(stdin, session_id, prompt_text, &["claude"]);
}

fn write_acp_router_review_prompt(
    stdin: &mut ChildStdin,
    session_id: &str,
    prompt_text: &str,
    candidates: &[&str],
) {
    write_acp_router_review_prompt_with_timeout(stdin, session_id, prompt_text, candidates, 5);
}

fn write_acp_router_review_prompt_with_timeout(
    stdin: &mut ChildStdin,
    session_id: &str,
    prompt_text: &str,
    candidates: &[&str],
    timeout_seconds: i64,
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
                "policy":{"candidates":candidates},
                "mode":"review",
                "timeoutSeconds":timeout_seconds
            }
        })
    )
    .unwrap();
}

fn make_executable(path: &Path) {
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).unwrap();
}

fn write_fake_provider_path(path: &Path, lines: &[&str]) {
    std::fs::write(path, lines.join("\n")).unwrap();
    make_executable(path);
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
fn stdio_binary_defaults_to_acp_router_without_starting_mcp_loop() {
    let env = fixture_env();
    let mut child = acp_router_command(&env)
        .spawn()
        .expect("spawn default ACP router");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    writeln!(
        stdin,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        })
    )
    .unwrap();
    stdin.flush().unwrap();

    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    let response: Value = serde_json::from_str(&line).unwrap();

    assert_eq!(response["id"], 1);
    assert_eq!(response["result"]["protocolVersion"], 1);
    assert!(response["result"]["agentCapabilities"].is_object());
    assert!(!env.state_dir.join("server.pid").exists());

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn default_acp_router_rejects_mcp_tools_list() {
    let env = fixture_env();
    let mut child = acp_router_command(&env)
        .spawn()
        .expect("spawn default ACP router");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    writeln!(
        stdin,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        })
    )
    .unwrap();
    stdin.flush().unwrap();

    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    let response: Value = serde_json::from_str(&line).unwrap();

    assert_eq!(response["id"], 1);
    assert_eq!(response["error"]["code"], -32601);
    assert_eq!(
        response["error"]["message"],
        "method not supported by Agent Bridge ACP router"
    );

    let _ = child.kill();
    let _ = child.wait();
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
    assert_eq!(router_result["verificationStatus"], "not_verified");
    assert!(!router_result["evidenceRefs"].as_array().unwrap().is_empty());
    assert_eq!(
        router_result["diagnostics"]["evidenceRefs"],
        router_result["evidenceRefs"]
    );
    let diagnostics = &router_result["diagnostics"];
    assert_eq!(diagnostics["provider"], "claude");
    assert_eq!(diagnostics["terminalKind"], "answer");
    assert_eq!(diagnostics["bounded"], true);
    assert_eq!(diagnostics["attempts"][0]["provider"], "claude");
    assert_eq!(diagnostics["failoverTrail"], json!([]));
    assert_eq!(
        diagnostics["evidenceRefs"][0]["agentId"],
        router_result["attempts"][0]["evidenceRef"]["agentId"]
    );
    for raw_key in ["stdout", "stderr", "transcript", "gitDiff"] {
        assert!(router_result.get(raw_key).is_none(), "{raw_key}");
        assert!(diagnostics.get(raw_key).is_none(), "{raw_key}");
    }
    assert!(env.log_dir.join("argv.txt").exists());

    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success());
}

#[test]
fn stdio_acp_router_prompt_fails_over_after_infrastructure_failure() {
    let env = fixture_env();
    let claude_provider = env.root.join("fake-claude-provider");
    let codex_provider = env.root.join("fake-codex-provider");
    write_fake_provider_path(
        &claude_provider,
        &[
            "#!/bin/sh",
            "case \"$*\" in",
            "  *--version*) echo fake-claude 1.0.0; exit 0 ;;",
            "esac",
            "read init || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{}}}'",
            "read new_session || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"sessionId\":\"claude-session\"}}'",
            "read prompt_request || exit 1",
            "sleep 2",
            "exit 0",
        ],
    );
    write_fake_provider_path(
        &codex_provider,
        &[
            "#!/bin/sh",
            "case \"$*\" in",
            "  *--version*) echo fake-codex 1.0.0; exit 0 ;;",
            "esac",
            "read init || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{}}}'",
            "read new_session || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"sessionId\":\"codex-session\"}}'",
            "read prompt_request || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"codex-session\",\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"codex recovered\"}}}}'",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"stopReason\":\"end_turn\"}}'",
            "exit 0",
        ],
    );

    let command = acp_router_command_with_bins(&env, &claude_provider, &codex_provider);
    let (mut child, mut stdin, mut stdout, session_id) =
        start_acp_router_session_with_command(&env, command);

    write_acp_router_review_prompt_with_timeout(
        &mut stdin,
        &session_id,
        "recover through fallback",
        &["claude", "codex"],
        1,
    );

    let (updates, response) = read_json_response(&mut stdout, 3);
    assert!(
        updates
            .iter()
            .all(|update| update["method"] == "session/update"),
        "{updates:?}"
    );
    assert_eq!(response["result"]["stopReason"], "end_turn");
    let router_result = &response["result"]["routerResult"];
    assert_eq!(router_result["provider"], "codex");
    assert_eq!(router_result["terminalKind"], "answer");
    assert_eq!(router_result["finalText"], "codex recovered");

    let attempts = router_result["attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0]["provider"], "claude");
    assert_eq!(attempts[0]["disposition"], "failover_eligible");
    assert_eq!(attempts[0]["failureCategory"], "provider_timeout");
    assert_eq!(attempts[1]["provider"], "codex");
    assert_eq!(attempts[1]["disposition"], "trusted_final");
    assert!(attempts[1]["failureCategory"].is_null());

    let diagnostics = &router_result["diagnostics"];
    assert_eq!(diagnostics["provider"], "codex");
    assert_eq!(diagnostics["terminalKind"], "answer");
    assert_eq!(diagnostics["attempts"].as_array().unwrap().len(), 2);
    let failover_trail = diagnostics["failoverTrail"].as_array().unwrap();
    assert_eq!(failover_trail.len(), 1);
    assert_eq!(failover_trail[0]["sourceProvider"], "claude");
    assert_eq!(failover_trail[0]["targetProvider"], "codex");
    assert_eq!(failover_trail[0]["failureCategory"], "provider_timeout");
    assert_eq!(failover_trail[0]["reason"], "failover_eligible");
    assert_eq!(failover_trail[0]["sourceAgentId"], attempts[0]["agentId"]);
    assert_eq!(failover_trail[0]["targetAgentId"], attempts[1]["agentId"]);
    let evidence_refs = diagnostics["evidenceRefs"].as_array().unwrap();
    assert_eq!(evidence_refs.len(), 2);
    assert_eq!(
        evidence_refs[0]["agentId"],
        attempts[0]["evidenceRef"]["agentId"]
    );
    assert_eq!(
        evidence_refs[1]["agentId"],
        attempts[1]["evidenceRef"]["agentId"]
    );

    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success());
}

#[test]
fn stdio_acp_router_prompt_returns_blocker_for_refusal_stop_reason() {
    let env = fixture_env();
    write_fake_provider_path(
        &env.fake_provider,
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
fn stdio_acp_router_prompt_returns_blocker_for_cancellation_without_failover() {
    let env = fixture_env();
    let claude_provider = env.root.join("fake-cancelled-claude-provider");
    let codex_provider = env.root.join("fake-marker-codex-provider");
    write_fake_provider_path(
        &claude_provider,
        &[
            "#!/bin/sh",
            "case \"$*\" in",
            "  *--version*) echo fake-claude 1.0.0; exit 0 ;;",
            "esac",
            "read init || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{}}}'",
            "read new_session || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"sessionId\":\"claude-session\"}}'",
            "read prompt_request || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"claude-session\",\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"partial cancellation text\"}}}}'",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"stopReason\":\"cancelled\"}}'",
            "exit 0",
        ],
    );
    write_fake_provider_path(
        &codex_provider,
        &[
            "#!/bin/sh",
            "case \"$*\" in",
            "  *--version*) echo fake-codex 1.0.0; exit 0 ;;",
            "esac",
            "printf called > \"$AGENT_BRIDGE_STATE_DIR/codex-called\"",
            "read init || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{}}}'",
            "read new_session || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"sessionId\":\"codex-session\"}}'",
            "read prompt_request || exit 1",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"stopReason\":\"end_turn\"}}'",
            "exit 0",
        ],
    );

    let command = acp_router_command_with_bins(&env, &claude_provider, &codex_provider);
    let (mut child, mut stdin, mut stdout, session_id) =
        start_acp_router_session_with_command(&env, command);

    write_acp_router_review_prompt(
        &mut stdin,
        &session_id,
        "cancel without fallback",
        &["claude", "codex"],
    );

    let (_updates, response) = read_json_response(&mut stdout, 3);
    assert_eq!(response["result"]["stopReason"], "cancelled");
    let router_result = &response["result"]["routerResult"];
    assert_eq!(router_result["provider"], "claude");
    assert_eq!(router_result["terminalKind"], "blocker");
    assert_eq!(router_result["blockerReason"], "cancelled");
    assert!(router_result["finalText"].is_null());
    assert_eq!(router_result["attempts"].as_array().unwrap().len(), 1);
    assert_eq!(router_result["attempts"][0]["disposition"], "blocker");
    assert_eq!(router_result["diagnostics"]["failoverTrail"], json!([]));
    assert!(!env.state_dir.join("codex-called").exists());

    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success());
}

#[test]
fn stdio_acp_router_prompt_returns_blocker_for_claude_auth_and_billing_without_failover() {
    for (case, stop_failure, failure_category) in [
        (
            "auth",
            "printf '%s\\n' '{\"session_id\":\"fake-session\",\"transcript_path\":\"/tmp/agent-bridge-fake-claude/auth.jsonl\",\"cwd\":\"/repo\",\"hook_event_name\":\"StopFailure\",\"error\":\"authentication_failed\",\"error_details\":\"OAuth refresh token is no longer valid\",\"last_assistant_message\":\"Session expired. Please run /login to sign in again.\"}' >&2",
            "claude_auth_error",
        ),
        (
            "billing",
            "printf '%s\\n' '{\"session_id\":\"fake-session\",\"transcript_path\":\"/tmp/agent-bridge-fake-claude/billing.jsonl\",\"cwd\":\"/repo\",\"hook_event_name\":\"StopFailure\",\"error\":\"billing_error\",\"error_details\":\"subscription or credit access unavailable\",\"last_assistant_message\":\"Your account does not have access to Claude. Please login again or contact your administrator.\"}' >&2",
            "claude_billing_error",
        ),
    ] {
        let env = fixture_env();
        let claude_provider = env.root.join(format!("fake-{case}-claude-provider"));
        let codex_provider = env.root.join(format!("fake-{case}-codex-provider"));
        write_fake_provider_path(
            &claude_provider,
            &[
                "#!/bin/sh",
                "case \"$*\" in",
                "  *--version*) echo fake-claude 1.0.0; exit 0 ;;",
                "esac",
                "read init || exit 1",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{}}}'",
                "read new_session || exit 1",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"sessionId\":\"claude-session\"}}'",
                "read prompt_request || exit 1",
                stop_failure,
                "echo 'not-json-from-provider'",
                "exit 0",
            ],
        );
        write_fake_provider_path(
            &codex_provider,
            &[
                "#!/bin/sh",
                "case \"$*\" in",
                "  *--version*) echo fake-codex 1.0.0; exit 0 ;;",
                "esac",
                "printf called > \"$AGENT_BRIDGE_STATE_DIR/codex-called\"",
                "read init || exit 1",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{}}}'",
                "read new_session || exit 1",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"sessionId\":\"codex-session\"}}'",
                "read prompt_request || exit 1",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"stopReason\":\"end_turn\"}}'",
                "exit 0",
            ],
        );

        let command = acp_router_command_with_bins(&env, &claude_provider, &codex_provider);
        let (mut child, mut stdin, mut stdout, session_id) =
            start_acp_router_session_with_command(&env, command);

        write_acp_router_review_prompt(
            &mut stdin,
            &session_id,
            "auth billing blocker",
            &["claude", "codex"],
        );

        let (_updates, response) = read_json_response(&mut stdout, 3);
        assert_eq!(response["result"]["stopReason"], "end_turn", "{case}");
        let router_result = &response["result"]["routerResult"];
        assert_eq!(router_result["provider"], "claude", "{case}");
        assert_eq!(router_result["terminalKind"], "blocker", "{case}");
        assert_eq!(router_result["failureCategory"], failure_category, "{case}");
        assert_eq!(router_result["blockerReason"], failure_category, "{case}");
        assert!(router_result["finalText"].is_null(), "{case}");
        assert_eq!(
            router_result["attempts"][0]["disposition"], "blocker",
            "{case}"
        );
        assert_eq!(router_result["diagnostics"]["failoverTrail"], json!([]));
        assert!(!env.state_dir.join("codex-called").exists(), "{case}");

        drop(stdin);
        let status = child.wait().unwrap();
        assert!(status.success(), "{case}");
    }
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
            "workspaces = [\"{}\"]\nstate_dir = \"{}\"\nmax_active_tasks = 7\nstrict_validation = true\n",
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
    assert_eq!(value["strictValidation"], true);
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
fn mcp_adapter_lists_only_delegate_and_evidence_tools() {
    let env = FixtureEnv::new("mcp-adapter-tool-list");
    let mut client = McpClient::start_with_command(mcp_adapter_command(&env));
    let response = client.request("tools/list", json!({}));
    let tools = response["result"]["tools"].as_array().unwrap();
    let names: Vec<_> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();

    assert_eq!(names, vec!["agent_delegate", "agent_evidence"]);
}

#[test]
fn mcp_adapter_agent_evidence_reads_bounded_sections() {
    let env = FixtureEnv::new("mcp-adapter-agent-evidence");
    let mut client = McpClient::start_with_command(mcp_adapter_command(&env));

    let result = client.tool(
        "agent_delegate",
        json!({
            "prompt": "review this change",
            "cwd": env.root,
            "mode": "review",
            "timeoutSeconds": 5,
            "policy": {"candidates": ["codex"]}
        }),
    );
    let evidence_ref = result["evidenceRefs"][0].clone();
    let evidence = client.tool(
        "agent_evidence",
        json!({
            "evidenceRef": evidence_ref,
            "sections": ["summary", "transcript"],
            "limit": 5,
            "maxBytes": 4096
        }),
    );

    assert_eq!(evidence["agentId"], result["evidenceRefs"][0]["agentId"]);
    assert!(evidence["reviewPacket"].is_object());
    assert!(evidence["transcript"]["events"].as_array().unwrap().len() <= 5);
}

#[test]
fn mcp_adapter_agent_delegate_returns_router_result() {
    let env = FixtureEnv::new("mcp-adapter-agent-delegate");
    let mut client = McpClient::start_with_command(mcp_adapter_command(&env));

    let result = client.tool(
        "agent_delegate",
        json!({
            "prompt": "review this change",
            "cwd": env.root,
            "mode": "review",
            "timeoutSeconds": 5,
            "policy": {"candidates": ["codex"]}
        }),
    );

    assert_eq!(result["terminalKind"], "answer");
    assert_eq!(result["provider"], "codex");
    assert_eq!(result["verificationStatus"], "not_verified");
    assert!(result["finalText"].as_str().unwrap().contains("fixture ok"));
    assert!(!result["evidenceRefs"].as_array().unwrap().is_empty());
    assert_eq!(
        result["diagnostics"]["evidenceRefs"],
        result["evidenceRefs"]
    );
}
