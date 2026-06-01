use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::ffi::OsString;
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

impl McpClient {
    fn start(env: &FixtureEnv) -> Self {
        Self::start_with_claude_env(env, false)
    }

    fn start_with_native_claude(env: &FixtureEnv) -> Self {
        Self::start_with_claude_env(env, true)
    }

    fn start_with_claude_env(env: &FixtureEnv, native_claude: bool) -> Self {
        let workspaces = std::env::join_paths([env.root.as_os_str()]).unwrap();
        Self::start_with_options(env, native_claude, Some(workspaces), None, BTreeMap::new())
    }

    fn start_with_workspace_value(env: &FixtureEnv, workspaces: OsString) -> Self {
        Self::start_with_options(env, false, Some(workspaces), None, BTreeMap::new())
    }

    fn start_with_legacy_allowed_root_only(env: &FixtureEnv) -> Self {
        Self::start_with_options(env, false, None, Some(env.root.clone()), BTreeMap::new())
    }

    fn start_with_extra_env(env: &FixtureEnv, extra_env: BTreeMap<String, OsString>) -> Self {
        let workspaces = std::env::join_paths([env.root.as_os_str()]).unwrap();
        Self::start_with_options(env, false, Some(workspaces), None, extra_env)
    }

    fn start_with_options(
        env: &FixtureEnv,
        native_claude: bool,
        workspaces: Option<OsString>,
        legacy_allowed_root: Option<PathBuf>,
        extra_env: BTreeMap<String, OsString>,
    ) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_agent-bridge-mcp"));
        command
            .env_remove("AGENT_BRIDGE_ALLOWED_ROOT")
            .env_remove("AGENT_BRIDGE_WORKSPACES")
            .env("AGENT_BRIDGE_STATE_DIR", &env.state_dir)
            .env("CURSOR_AGENT_BIN", &env.fake_provider)
            .env("PI_BIN", &env.fake_provider)
            .env("CODEX_BIN", &env.fake_provider)
            .env("ANTHROPIC_API_KEY", "test-key")
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
        if native_claude {
            command.env("CLAUDE_BIN", &env.fake_provider);
        } else {
            command.env("CLAUDE_P_BIN", &env.fake_provider);
        }
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

        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        assert!(!line.is_empty(), "expected MCP response for id={id}");
        let response: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(response["id"], id);
        response
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
            "stdin=$(cat)",
            "if [ -n \"$AGENT_BRIDGE_STATE_DIR\" ]; then",
            "  mkdir -p \"$AGENT_BRIDGE_STATE_DIR/provider-log\"",
            "  printf '%s\\n' \"$*\" > \"$AGENT_BRIDGE_STATE_DIR/provider-log/argv.txt\"",
            "  printf '%s' \"$stdin\" > \"$AGENT_BRIDGE_STATE_DIR/provider-log/stdin.txt\"",
            "fi",
            "if [ \"$1\" = \"--version\" ]; then",
            "  echo fake-provider 1.0.0",
            "  exit 0",
            "fi",
            "case \"$stdin\" in",
            "  *echo-api-key*)",
            "    echo \"$ANTHROPIC_API_KEY\"",
            "    exit 0",
            "    ;;",
            "  *claude-timeout*)",
            "    echo claude-task-started",
            "    sleep 2 &",
            "    child=$!",
            "    trap 'kill -TERM \"$child\" 2>/dev/null || true; wait \"$child\" 2>/dev/null || true; exit 143' TERM INT",
            "    wait \"$child\"",
            "    ;;",
            "  *non-zero-exit*)",
            "    echo 'provider refused task' >&2",
            "    exit 42",
            "    ;;",
            "  *missing-result*)",
            "    echo '{\"type\":\"result\"}'",
            "    exit 0",
            "    ;;",
            "  *terminal-noise*)",
            "    echo 'terminal probe noise'",
            "    printf '%s\\n' '{\"type\":\"result\",\"subtype\":\"success\",\"result\":\"fixture ok\"}'",
            "    echo 'trailing noise'",
            "    exit 0",
            "    ;;",
            "  *secret-token-for-redaction*)",
            "    echo 'secret-token-for-redaction'",
            "    exit 0",
            "    ;;",
            "  *malformed-output*)",
            "    echo 'not-json-from-claude'",
            "    echo 'terminal noise' >&2",
            "    exit 0",
            "    ;;",
            "  *AGENT_BRIDGE_PROVIDER_SMOKE_OK*)",
            "    printf '%s\\n' '{\"type\":\"result\",\"subtype\":\"success\",\"result\":\"AGENT_BRIDGE_PROVIDER_SMOKE_OK\"}'",
            "    exit 0",
            "    ;;",
            "esac",
            "if [ -n \"$stdin\" ]; then",
            "  printf '%s\\n' '{\"type\":\"result\",\"subtype\":\"success\",\"result\":\"fixture ok\"}'",
            "  exit 0",
            "fi",
            "case \"$*\" in",
            "  *sleep-long*)",
            "    echo started-long",
            "    echo waiting-long >&2",
            "    sleep 2 &",
            "    child=$!",
            "    trap 'kill -TERM \"$child\" 2>/dev/null || true; wait \"$child\" 2>/dev/null || true; exit 143' TERM INT",
            "    wait \"$child\"",
            "    ;;",
            "  *emit-logs*)",
            "    echo lifecycle-stdout",
            "    echo lifecycle-stderr >&2",
            "    exit 0",
            "    ;;",
            "esac",
            "printf '%s\\n' \"$*\"",
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

    let tools = client.request("tools/list", json!({}));
    let tools = tools["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 11);
    let task_preview = tools
        .iter()
        .find(|tool| tool["name"] == "task_preview")
        .unwrap();
    let providers_check = tools
        .iter()
        .find(|tool| tool["name"] == "providers_check")
        .unwrap();
    assert_eq!(
        providers_check["inputSchema"]["properties"]["providers"]["items"]["enum"],
        json!(["claude", "cursor", "kimi", "codex"])
    );
    assert_eq!(
        providers_check["inputSchema"]["properties"]["aggregateTimeoutMs"]["maximum"],
        120000
    );
    assert_eq!(
        providers_check["inputSchema"]["properties"]["providerTimeoutMs"]["additionalProperties"]["maximum"],
        90000
    );
    assert_eq!(
        task_preview["inputSchema"]["properties"]["provider"]["enum"],
        json!(["claude", "cursor", "kimi", "codex"])
    );
    assert_eq!(
        task_preview["inputSchema"]["required"],
        json!(["provider", "mode", "prompt"])
    );
    assert_eq!(
        task_preview["inputSchema"]["additionalProperties"],
        json!(false)
    );

    let prompts = client.request("prompts/list", json!({}));
    let prompts = prompts["result"]["prompts"].as_array().unwrap();
    assert_eq!(prompts.len(), 4);
    assert!(
        prompts
            .iter()
            .any(|prompt| prompt["name"] == "agent_bridge_delegate_implementation")
    );

    let prompt = client.request(
        "prompts/get",
        json!({"name": "agent_bridge_delegate_implementation"}),
    );
    let prompt_text = prompt["result"]["messages"][0]["content"]["text"]
        .as_str()
        .unwrap();
    assert!(prompt_text.contains("task_spawn"));
    assert!(prompt_text.contains("main caller remains responsible"));

    let resources = client.request("resources/list", json!({}));
    let resources = resources["result"]["resources"].as_array().unwrap();
    assert_eq!(resources.len(), 3);
    assert!(
        resources
            .iter()
            .any(|resource| resource["uri"] == "agent-bridge://guidance/caller-workflow")
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
            .contains("task_result")
    );

    let missing_resource = client.request("resources/read", json!({"uri": "file:///etc/passwd"}));
    assert_eq!(missing_resource["error"]["code"], -32002);

    let missing = client.request("missing/method", json!({}));
    assert_eq!(missing["error"]["code"], -32601);
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
        vec!["claude", "cursor", "kimi", "codex"]
    );

    let preview = client.tool_with_meta(
        "task_preview",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "secret prompt",
            "cwd": env.root
        }),
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
        "name": "task_preview",
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
        vec!["claude", "cursor", "kimi", "codex"]
    );

    let checks = client.tool("providers_check", json!({"smoke": true, "timeoutMs": 5000}));
    assert_eq!(
        checks["providers"]["codex"]["version"],
        "fake-provider 1.0.0"
    );
    assert_eq!(checks["providers"]["claude"]["startupVerified"], true);

    let preview = client.tool(
        "task_preview",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "secret prompt",
            "cwd": env.root
        }),
    );
    assert_eq!(
        preview["command"].as_str().unwrap(),
        env.fake_provider.to_string_lossy()
    );
    assert_eq!(preview["timeoutSeconds"], 120);
    assert!(
        preview["args"]
            .as_array()
            .unwrap()
            .iter()
            .any(|arg| arg == "<prompt redacted>")
    );

    let claude = client.tool(
        "task_preview",
        json!({
            "provider": "claude",
            "mode": "review",
            "prompt": "provider prompt",
            "cwd": env.root,
            "effort": "high"
        }),
    );
    assert_eq!(claude["command"], "/bin/zsh");
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

    let outside = temp_dir("agent-bridge-outside");
    let link = env.root.join("escape");
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    let error = client.tool_error(
        "task_preview",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "x",
            "cwd": link
        }),
    );
    assert!(error.contains("outside configured workspaces"));

    let error = client.tool_error(
        "task_preview",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "x".repeat(101 * 1024),
            "cwd": env.root
        }),
    );
    assert!(error.contains("prompt exceeds"));
}

#[test]
fn stdio_providers_check_filters_and_validates_readiness_inputs() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let filtered = client.tool(
        "providers_check",
        json!({"smoke": false, "providers": ["cursor", "cursor"]}),
    );
    assert_eq!(provider_keys(&filtered), vec!["cursor"]);
    assert_eq!(
        filtered["providers"]["cursor"]["version"],
        "fake-provider 1.0.0"
    );
    assert!(filtered["providers"]["cursor"]["versionDurationMs"].is_number());
    assert!(
        filtered["providers"]["cursor"]
            .get("smokeDurationMs")
            .is_none()
    );

    let unknown_provider = client.tool_error("providers_check", json!({"providers": ["openai"]}));
    assert!(unknown_provider.contains("claude"));
    assert!(unknown_provider.contains("codex"));

    let empty_filter = client.tool_error("providers_check", json!({"providers": []}));
    assert!(empty_filter.contains("at least one provider"));

    let invalid_aggregate = client.tool_error(
        "providers_check",
        json!({"smoke": true, "aggregateTimeoutMs": 0}),
    );
    assert!(invalid_aggregate.contains("aggregateTimeoutMs"));

    let invalid_budget = client.tool_error(
        "providers_check",
        json!({"smoke": true, "providerTimeoutMs": {"cursor": 0}}),
    );
    assert!(invalid_budget.contains("providerTimeoutMs.cursor"));

    let unknown_budget_provider = client.tool_error(
        "providers_check",
        json!({"smoke": true, "providerTimeoutMs": {"openai": 1000}}),
    );
    assert!(unknown_budget_provider.contains("provider must be one of"));
}

#[test]
fn stdio_providers_check_uses_provider_budgets_and_concurrency() {
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
            "is_cursor=0",
            "is_kimi=0",
            "case \"$*\" in",
            "  *--workspace*) is_cursor=1 ;;",
            "  *--tools*) is_kimi=1 ;;",
            "esac",
            "if [ \"$is_cursor\" = 1 ] || [ \"$is_kimi\" = 1 ]; then",
            "  sleep 2",
            "  echo AGENT_BRIDGE_PROVIDER_SMOKE_OK",
            "  exit 0",
            "fi",
            "if printf '%s\\n%s\\n' \"$stdin\" \"$*\" | grep -q AGENT_BRIDGE_PROVIDER_SMOKE_OK; then",
            "  echo AGENT_BRIDGE_PROVIDER_SMOKE_OK",
            "  exit 0",
            "fi",
            "exit 0",
            "",
        ],
    );
    let mut client = McpClient::start(&env);
    let started = std::time::Instant::now();

    let checks = client.tool(
        "providers_check",
        json!({
            "smoke": true,
            "providers": ["cursor", "kimi"],
            "aggregateTimeoutMs": 4500,
            "providerTimeoutMs": {"cursor": 3000, "kimi": 3000}
        }),
    );

    assert_eq!(sorted_provider_keys(&checks), vec!["cursor", "kimi"]);
    assert_eq!(checks["providers"]["cursor"]["startupVerified"], true);
    assert_eq!(checks["providers"]["kimi"]["startupVerified"], true);
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
            "stdin=$(cat)",
            "if [ \"$1\" = \"--version\" ]; then",
            "  echo fake-provider 1.0.0",
            "  exit 0",
            "fi",
            "if printf '%s\\n%s\\n' \"$stdin\" \"$*\" | grep -q AGENT_BRIDGE_PROVIDER_SMOKE_OK; then",
            "  sleep 1",
            "  echo AGENT_BRIDGE_PROVIDER_SMOKE_OK",
            "  exit 0",
            "fi",
            "exit 0",
            "",
        ],
    );
    let mut client = McpClient::start(&env);
    let started = std::time::Instant::now();

    let checks = client.tool(
        "providers_check",
        json!({
            "smoke": true,
            "aggregateTimeoutMs": 5000,
            "providerTimeoutMs": {
                "claude": 3000,
                "cursor": 3000,
                "kimi": 3000,
                "codex": 3000
            }
        }),
    );

    assert_eq!(
        sorted_provider_keys(&checks),
        vec!["claude", "codex", "cursor", "kimi"]
    );
    for provider in ["claude", "cursor", "kimi", "codex"] {
        assert_eq!(checks["providers"][provider]["startupVerified"], true);
    }
    assert!(
        started.elapsed() < std::time::Duration::from_millis(3600),
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
            "stdin=$(cat)",
            "if [ \"$1\" = \"--version\" ]; then",
            "  echo fake-provider 1.0.0",
            "  exit 0",
            "fi",
            "case \"$*\" in",
            "  *--workspace*)",
            "    sleep 5 &",
            "    child=$!",
            "    printf '%s\\n' \"$child\" > \"$AGENT_BRIDGE_STATE_DIR/provider-log/smoke-child.pid\"",
            "    wait \"$child\"",
            "    ;;",
            "esac",
            "exit 0",
            "",
        ],
    );
    let mut client = McpClient::start(&env);

    let checks = client.tool(
        "providers_check",
        json!({"smoke": true, "providers": ["cursor"], "timeoutMs": 800}),
    );
    let cursor = &checks["providers"]["cursor"];
    assert_eq!(cursor["startupVerified"], false);
    assert_eq!(cursor["diagnostic"]["failureCategory"], "provider_timeout");
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
        "providers_check",
        json!({
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
            "stdin=$(cat)",
            "if [ \"$1\" = \"--version\" ]; then",
            "  echo fake-provider 1.0.0",
            "  exit 0",
            "fi",
            "case \"$*\" in",
            "  *--workspace*|*--tools*) sleep 1; echo AGENT_BRIDGE_PROVIDER_SMOKE_OK; exit 0 ;;",
            "esac",
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
        "providers_check",
        json!({
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

    let mut extra_env = BTreeMap::new();
    extra_env.insert(
        "AGENT_BRIDGE_SMOKE_CONCURRENCY".to_string(),
        OsString::from("not-a-number"),
    );
    let mut client = McpClient::start_with_extra_env(&env, extra_env);
    let started = std::time::Instant::now();
    let checks = client.tool(
        "providers_check",
        json!({
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

    let checks = client.tool("providers_check", json!({"providers": ["codex"]}));
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
        "task_preview",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "x",
            "cwd": second_root
        }),
    );
    assert_eq!(
        preview["cwd"].as_str().unwrap(),
        second_root.canonicalize().unwrap().to_str().unwrap()
    );

    let outside = temp_dir("agent-bridge-outside");
    let error = client.tool_error(
        "task_preview",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "x",
            "cwd": outside
        }),
    );
    assert!(error.contains("outside configured workspaces"));
}

#[test]
fn stdio_ignores_legacy_allowed_root_env_var() {
    let env = fixture_env();
    let mut client = McpClient::start_with_legacy_allowed_root_only(&env);

    let error = client.tool_error(
        "task_preview",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "x",
            "cwd": env.root
        }),
    );
    assert!(error.contains("outside configured workspaces"));
}

#[test]
fn stdio_task_lifecycle_stop_timeout_and_logs() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let spawned = client.tool(
        "task_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "emit-logs",
            "cwd": env.root
        }),
    );
    let task_id = spawned["taskId"].as_str().unwrap();
    assert!(task_id.starts_with("task_"));

    let waited = client.tool("task_wait", json!({"taskId": task_id, "timeoutMs": 2000}));
    assert_eq!(waited["status"], "succeeded");

    let logs = client.tool("task_logs", json!({"taskId": task_id}));
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

    let result = client.tool("task_result", json!({"taskId": task_id}));
    assert_eq!(result["status"], "succeeded");
    assert_eq!(result["exitCode"], 0);

    let active = client.tool(
        "task_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "sleep-long",
            "cwd": env.root,
            "timeoutSeconds": 20
        }),
    );
    let active_id = active["taskId"].as_str().unwrap();
    let remove_error = client.tool_error("task_remove", json!({"taskId": active_id}));
    assert!(remove_error.contains("cannot remove a running task"));

    let stopped = client.tool("task_stop", json!({"taskId": active_id}));
    assert_eq!(stopped["status"], "stopped");

    let timed = client.tool(
        "task_spawn",
        json!({
            "provider": "codex",
            "mode": "review",
            "prompt": "sleep-long",
            "cwd": env.root,
            "timeoutSeconds": 1
        }),
    );
    let timed_id = timed["taskId"].as_str().unwrap();
    let timed_wait = client.tool("task_wait", json!({"taskId": timed_id, "timeoutMs": 3000}));
    assert_eq!(timed_wait["status"], "failed");
    assert_eq!(timed_wait["errorType"], "timeout");
}

#[test]
fn stdio_claude_task_prompt_is_passed_on_stdin_not_argv() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);
    let prompt = "--leading-flag\nquoted \"value\" $(touch should-not-run) secret-token";

    let spawned = client.tool(
        "task_spawn",
        json!({
            "provider": "claude",
            "mode": "review",
            "prompt": prompt,
            "cwd": env.root
        }),
    );
    let task_id = spawned["taskId"].as_str().unwrap();
    let waited = client.tool("task_wait", json!({"taskId": task_id, "timeoutMs": 3000}));
    assert_eq!(waited["status"], "succeeded");

    let argv = std::fs::read_to_string(env.log_dir.join("argv.txt")).unwrap();
    let stdin = std::fs::read_to_string(env.log_dir.join("stdin.txt")).unwrap();
    assert!(
        !argv.contains(prompt),
        "rendered Claude prompt leaked into argv: {argv}"
    );
    assert!(
        stdin.contains(prompt),
        "rendered Claude prompt was not delivered on stdin: {stdin}"
    );
}

#[test]
fn stdio_claude_prompt_survives_stdin_consuming_zsh_startup_files() {
    let env = fixture_env();
    let home = temp_dir("agent-bridge-home");
    std::fs::write(home.join(".zshenv"), "cat >/dev/null || true\n").unwrap();
    let mut extra_env = BTreeMap::new();
    extra_env.insert("HOME".to_string(), home.into_os_string());
    let mut client = McpClient::start_with_extra_env(&env, extra_env);

    let spawned = client.tool(
        "task_spawn",
        json!({
            "provider": "claude",
            "mode": "review",
            "prompt": "terminal-noise",
            "cwd": env.root,
            "timeoutSeconds": 5
        }),
    );
    let task_id = spawned["taskId"].as_str().unwrap();

    let waited = client.tool("task_wait", json!({"taskId": task_id, "timeoutMs": 3000}));
    assert_eq!(waited["status"], "succeeded");

    let result = client.tool("task_result", json!({"taskId": task_id}));
    assert!(
        result["stdout"]
            .as_str()
            .unwrap()
            .contains("terminal probe noise"),
        "provider did not receive prompt on stdin: {result}"
    );
}

#[test]
fn stdio_native_claude_bin_selection_uses_native_print_args() {
    let env = fixture_env();
    let mut client = McpClient::start_with_native_claude(&env);

    let preview = client.tool(
        "task_preview",
        json!({
            "provider": "claude",
            "mode": "review",
            "prompt": "native prompt",
            "cwd": env.root
        }),
    );
    let args = preview["args"].as_array().unwrap();
    assert_eq!(preview["command"], "/bin/zsh");
    assert!(
        args.iter()
            .any(|arg| arg.as_str() == Some(env.fake_provider.to_str().unwrap()))
    );
    assert!(args.iter().any(|arg| arg == "-p"));
    assert!(!args.iter().any(|arg| arg == "--cwd"));
    assert_eq!(preview["stdin"], "<prompt redacted>");
}

#[test]
fn stdio_claude_smoke_timeout_returns_bounded_diagnostic() {
    let env = fixture_env();
    std::fs::write(
        &env.fake_provider,
        [
            "#!/bin/sh",
            "stdin=$(cat)",
            "if [ \"$1\" = \"--version\" ]; then",
            "  echo fake-provider 1.0.0",
            "  exit 0",
            "fi",
            "echo claude-p booting",
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

    let checks = client.tool("providers_check", json!({"smoke": true, "timeoutMs": 500}));
    let claude = &checks["providers"]["claude"];
    assert_eq!(claude["available"], false);
    assert_eq!(claude["startupVerified"], false);
    assert_eq!(claude["diagnostic"]["failureCategory"], "provider_timeout");
    assert_eq!(claude["diagnostic"]["timeoutMs"], 500);
    assert!(
        claude["diagnostic"]["stdoutExcerpt"]
            .as_str()
            .unwrap()
            .contains("claude-p booting")
    );
    assert!(
        claude["diagnostic"]["stderrExcerpt"]
            .as_str()
            .unwrap()
            .contains("waiting for stop hook")
    );
}

#[test]
fn stdio_claude_task_malformed_output_returns_diagnostic() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let spawned = client.tool(
        "task_spawn",
        json!({
            "provider": "claude",
            "mode": "review",
            "prompt": "malformed-output",
            "cwd": env.root
        }),
    );
    let task_id = spawned["taskId"].as_str().unwrap();
    let waited = client.tool("task_wait", json!({"taskId": task_id, "timeoutMs": 3000}));
    assert_eq!(waited["status"], "failed");
    assert_eq!(waited["errorType"], "provider_output_error");

    let result = client.tool("task_result", json!({"taskId": task_id}));
    assert_eq!(
        result["diagnostic"]["failureCategory"],
        "provider_output_error"
    );
    assert!(
        result["diagnostic"]["stdoutExcerpt"]
            .as_str()
            .unwrap()
            .contains("not-json-from-claude")
    );
    assert!(
        result["diagnostic"]["stderrExcerpt"]
            .as_str()
            .unwrap()
            .contains("terminal noise")
    );
}

#[test]
fn stdio_claude_task_failure_modes_are_classified() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let cases = [
        ("non-zero-exit", "provider_exit_error", 5),
        ("missing-result", "provider_output_error", 5),
        ("claude-timeout", "timeout", 1),
    ];
    for (prompt, error_type, timeout_seconds) in cases {
        let spawned = client.tool(
            "task_spawn",
            json!({
                "provider": "claude",
                "mode": "review",
                "prompt": prompt,
                "cwd": env.root,
                "timeoutSeconds": timeout_seconds
            }),
        );
        let task_id = spawned["taskId"].as_str().unwrap();
        let waited = client.tool("task_wait", json!({"taskId": task_id, "timeoutMs": 3000}));
        assert_eq!(waited["status"], "failed", "{prompt}");
        assert_eq!(waited["errorType"], error_type, "{prompt}");

        let result = client.tool("task_result", json!({"taskId": task_id}));
        assert!(
            result["diagnostic"]["failureCategory"].is_string(),
            "{prompt}"
        );
    }
}

#[test]
fn stdio_claude_task_extracts_result_with_surrounding_noise() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let spawned = client.tool(
        "task_spawn",
        json!({
            "provider": "claude",
            "mode": "review",
            "prompt": "terminal-noise",
            "cwd": env.root
        }),
    );
    let task_id = spawned["taskId"].as_str().unwrap();
    let waited = client.tool("task_wait", json!({"taskId": task_id, "timeoutMs": 3000}));
    assert_eq!(waited["status"], "succeeded");
}

#[test]
fn stdio_claude_task_diagnostic_redacts_prompt_content() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let spawned = client.tool(
        "task_spawn",
        json!({
            "provider": "claude",
            "mode": "review",
            "prompt": "please handle secret-token-for-redaction",
            "cwd": env.root
        }),
    );
    let task_id = spawned["taskId"].as_str().unwrap();
    let waited = client.tool("task_wait", json!({"taskId": task_id, "timeoutMs": 3000}));
    assert_eq!(waited["status"], "failed");

    let result = client.tool("task_result", json!({"taskId": task_id}));
    let diagnostic = serde_json::to_string(&result["diagnostic"]).unwrap();
    assert!(
        !diagnostic.contains("secret-token-for-redaction"),
        "diagnostic leaked prompt content: {diagnostic}"
    );
}

#[test]
fn stdio_claude_task_diagnostic_redacts_token_values() {
    let env = fixture_env();
    let mut client = McpClient::start(&env);

    let spawned = client.tool(
        "task_spawn",
        json!({
            "provider": "claude",
            "mode": "review",
            "prompt": "echo-api-key",
            "cwd": env.root
        }),
    );
    let task_id = spawned["taskId"].as_str().unwrap();
    let waited = client.tool("task_wait", json!({"taskId": task_id, "timeoutMs": 3000}));
    assert_eq!(waited["status"], "failed");

    let result = client.tool("task_result", json!({"taskId": task_id}));
    let diagnostic = serde_json::to_string(&result["diagnostic"]).unwrap();
    assert!(!diagnostic.contains("test-key"), "diagnostic leaked token");
}

#[test]
fn stdio_managed_worktree_lifecycle() {
    let env = fixture_env();
    let repo = env.root.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    init_git_repo(&repo);

    let mut client = McpClient::start(&env);
    let spawned = client.tool(
        "task_spawn",
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
    let task_id = spawned["taskId"].as_str().unwrap();
    let worktree_path = PathBuf::from(spawned["worktreePath"].as_str().unwrap());
    assert!(worktree_path.exists());

    let waited = client.tool("task_wait", json!({"taskId": task_id, "timeoutMs": 3000}));
    assert_eq!(waited["status"], "succeeded");

    let result = client.tool("task_result", json!({"taskId": task_id}));
    assert_eq!(result["gitStatus"], "");
    assert_eq!(result["changedFiles"], json!([]));

    let removed = client.tool("task_remove", json!({"taskId": task_id}));
    assert_eq!(removed["status"], "removed");
    assert!(!worktree_path.exists());
}
