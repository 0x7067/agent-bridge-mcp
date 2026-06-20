# ACP Core MCP Adapter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `agent-bridge-mcp` run the ACP router by default while keeping only a tiny `mcp-adapter` subcommand with `agent_delegate` and `agent_evidence` for Codex and Claude Code.

**Architecture:** The default binary becomes an ACP JSON-RPC runtime for `initialize`, `session/new`, and `session/prompt`. The MCP adapter is a protocol shim that calls the same in-process router turn and evidence reader; it does not expose spawn/poll/stop/remove lifecycle operations. The task manager, provider adapters, transcripts, and result section readers remain internal reliability primitives.

**Tech Stack:** Rust 2024, Tokio async runtime, serde/serde_json for newline-delimited JSON-RPC, clap for subcommands, deterministic fake providers in `crates/agent-bridge-mcp/tests/stdio_binary.rs`.

## Global Constraints

- No legacy eight-tool MCP compatibility: remove public `agent_spawn`, `agent_observe`, `agent_result`, `agent_list`, `agent_stop`, and `agent_remove`.
- Keep MCP only as `agent-bridge-mcp mcp-adapter`.
- The adapter exposes only `agent_delegate` and `agent_evidence`.
- Every terminal routed result includes `verificationStatus: "not_verified"`.
- Provider output remains evidence only; callers still own project verification.
- Do not rewrite task manager, provider adapters, transcript capture, or worktree isolation in this change.
- Do not rename the crate or installed binary in this change.
- Use deterministic fake-provider stdio tests; do not add new dependencies.
- Preserve unrelated worktree changes and report them separately.
- Verification gate before done: run `cargo test -p agent-bridge-mcp --test stdio_binary -- --test-threads=1`, `cargo test -p agent-bridge-mcp`, and `scripts/quality.sh`.

---

## File Structure

- Modify `crates/agent-bridge-mcp/src/runtime.rs`: CLI routing only, default command dispatch, config/doctor/reload/pid handling, and calls into `router_runtime` and `mcp_adapter`.
- Create `crates/agent-bridge-mcp/src/router_runtime.rs`: ACP router stdio loop, ACP request parsing, reusable routed prompt execution, router result JSON construction, and ACP `session/update` notifications.
- Create `crates/agent-bridge-mcp/src/mcp_adapter.rs`: minimal MCP stdio server with `initialize`, `tools/list`, `tools/call`, `agent_delegate`, and `agent_evidence`.
- Modify `crates/agent-bridge-mcp/src/server.rs`: remove old MCP request handling and lifecycle tool dispatch; keep diagnostics/doctor support internal to the crate.
- Delete `crates/agent-bridge-mcp/src/guidance.rs`: old MCP prompts/resources are removed from the product surface.
- Delete `crates/agent-bridge-mcp/src/tools.rs`: old lifecycle MCP tool names/schemas are removed after moving `TaskPreviewInput`.
- Modify `crates/agent-bridge-mcp/src/task/spawn.rs`: import `TaskPreviewInput` from the new task-local module.
- Create `crates/agent-bridge-mcp/src/task/input.rs`: task spawn input structs used internally by the task manager.
- Modify `crates/agent-bridge-mcp/src/task.rs`: expose `task::input::TaskPreviewInput` within the crate and keep `ResultSections` public for adapter evidence reads.
- Modify `crates/agent-bridge-mcp/src/lib.rs`: export `mcp_adapter` and `router_runtime`, stop publicly exporting `guidance`, `server`, and `tools`.
- Modify `crates/agent-bridge-mcp/tests/stdio_binary.rs`: update default runtime tests to ACP, add adapter tests, and remove tests whose only purpose is the deleted lifecycle MCP server.
- Replace `crates/agent-bridge-mcp/tests/server_protocol.rs` with `crates/agent-bridge-mcp/tests/mcp_adapter_protocol.rs`: unit-level protocol tests for the two adapter tools.
- Modify `crates/agent-bridge-mcp/tests/protocol_models.rs`: remove old `ToolName`/`ToolCallParams` assertions and point spawn-input parsing at `task::input::TaskPreviewInput`.
- Modify `README.md` and `docs/agents/architecture.md`: document ACP default and the two-tool MCP adapter.

---

### Task 1: Make The Default Binary ACP Router

**Files:**
- Modify: `crates/agent-bridge-mcp/src/runtime.rs`
- Modify: `crates/agent-bridge-mcp/tests/stdio_binary.rs`

**Interfaces:**
- Consumes: existing `run_acp_router()` behavior from `runtime.rs`.
- Produces: default `agent-bridge-mcp` invocation runs ACP JSON-RPC and does not acquire `server.pid`.

- [ ] **Step 1: Change the stdio test helper so ACP tests launch the default binary**

In `crates/agent-bridge-mcp/tests/stdio_binary.rs`, replace `acp_router_command_with_bins` with this version. The function intentionally does not add `acp-router`.

```rust
fn acp_router_command_with_bins(
    env: &FixtureEnv,
    claude_bin: &Path,
    codex_bin: &Path,
) -> Command {
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
```

- [ ] **Step 2: Replace the default-runtime assertion with an ACP assertion**

In `crates/agent-bridge-mcp/tests/stdio_binary.rs`, replace `stdio_binary_exposes_acp_router_runtime_without_starting_mcp_loop` with:

```rust
#[test]
fn stdio_binary_defaults_to_acp_router_without_starting_mcp_loop() {
    let env = FixtureEnv::new("default-acp-router");
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
```

- [ ] **Step 3: Add a default-runtime rejection test for MCP `tools/list`**

Add this test near the default-runtime assertion:

```rust
#[test]
fn default_acp_router_rejects_mcp_tools_list() {
    let env = FixtureEnv::new("default-acp-router-tools-list");
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
```

- [ ] **Step 4: Run the focused failing tests**

Run:

```bash
cargo test -p agent-bridge-mcp --test stdio_binary stdio_binary_defaults_to_acp_router_without_starting_mcp_loop default_acp_router_rejects_mcp_tools_list -- --test-threads=1
```

Expected: FAIL because default invocation still starts the old MCP stdio server and creates the pid lock.

- [ ] **Step 5: Change CLI dispatch so `None` runs ACP and remove `acp-router`**

In `crates/agent-bridge-mcp/src/runtime.rs`, remove `AcpRouter` from `CliCommand` and change `main_entry()` command handling to this structure:

```rust
    match cli.command {
        Some(CliCommand::Reload) => exit_with_reload(cli.config),
        Some(CliCommand::ClaudeHostRunner {
            socket: socket_path,
        }) => {
            if let Err(error) = crate::claude_host::run_server(socket_path).await {
                tracing::error!(error = %error, "[agent-bridge] fatal {error}");
                std::process::exit(1);
            }
            return;
        }
        None => {
            if let Err(error) = run_acp_router().await {
                tracing::error!(error = %error, "[agent-bridge] fatal {error}");
                std::process::exit(1);
            }
            return;
        }
    }
```

Keep the subcommand enum limited to non-adapter commands in this task:

```rust
#[derive(Subcommand, Debug)]
enum CliCommand {
    /// Ask a running Agent Bridge server to reload config from disk.
    Reload,
    /// Run the Agent Bridge-owned Claude host runner on the given Unix socket.
    ClaudeHostRunner { socket: std::path::PathBuf },
}
```

For this task only, keep the existing `run_acp_router()` function in `runtime.rs` and call it as `run_acp_router().await` if `crate::router_runtime` has not been created yet. Task 2 moves it.

- [ ] **Step 6: Remove old MCP stdio shutdown path from default execution**

Delete `run_until_shutdown()` and `run_stdio_server()` from `runtime.rs` after no call sites remain. Leave `shutdown_signal()` for `reload`/future daemon work only if the compiler still finds a use; otherwise remove it in the same edit.

- [ ] **Step 7: Run the focused tests**

Run:

```bash
cargo test -p agent-bridge-mcp --test stdio_binary stdio_binary_defaults_to_acp_router_without_starting_mcp_loop default_acp_router_rejects_mcp_tools_list -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/agent-bridge-mcp/src/runtime.rs crates/agent-bridge-mcp/tests/stdio_binary.rs
git commit -m "feat: make acp router the default runtime"
```

---

### Task 2: Extract Reusable Router Runtime And Add Verification Status

**Files:**
- Create: `crates/agent-bridge-mcp/src/router_runtime.rs`
- Modify: `crates/agent-bridge-mcp/src/runtime.rs`
- Modify: `crates/agent-bridge-mcp/src/lib.rs`
- Modify: `crates/agent-bridge-mcp/tests/stdio_binary.rs`

**Interfaces:**
- Consumes: default ACP runtime from Task 1.
- Produces:
  - `pub async fn run_acp_router() -> tokio::io::Result<()>`
  - `pub(crate) struct RouterPromptTurn`
  - `pub(crate) enum RouterUpdateSink<'a>`
  - `pub(crate) struct RouterTerminalResult`
  - `pub(crate) enum RouterPromptError`
  - `pub(crate) async fn execute_router_turn(turn: RouterPromptTurn, mut updates: RouterUpdateSink<'_>) -> Result<RouterTerminalResult, RouterPromptError>`
  - Every `routerResult` has top-level `evidenceRefs` and `verificationStatus: "not_verified"`.

- [ ] **Step 1: Add router result assertions to the existing ACP prompt test**

In `stdio_acp_router_prompt_runs_one_provider_turn`, after the existing `routerResult` assertions, add:

```rust
    let router_result = &prompt_response["result"]["routerResult"];
    assert_eq!(router_result["verificationStatus"], "not_verified");
    assert!(router_result["evidenceRefs"].as_array().unwrap().len() >= 1);
    assert_eq!(
        router_result["diagnostics"]["evidenceRefs"],
        router_result["evidenceRefs"]
    );
```

- [ ] **Step 2: Run the focused failing test**

Run:

```bash
cargo test -p agent-bridge-mcp --test stdio_binary stdio_acp_router_prompt_runs_one_provider_turn -- --test-threads=1
```

Expected: FAIL because the router result does not yet include top-level `verificationStatus` or `evidenceRefs`.

- [ ] **Step 3: Create `router_runtime.rs` by moving ACP-only code out of `runtime.rs`**

Create `crates/agent-bridge-mcp/src/router_runtime.rs` with the imports and public entry points below. Move the current `run_acp_router`, `RouterSession`, `handle_acp_router_request`, ACP param structs, prompt-text helpers, router classification helpers, notification writers, and `write_json_message` from `runtime.rs` into this file.

```rust
use crate::domain::{FailureCategory, ProviderKind, TaskMode};
use crate::mcp::{JsonRpcId, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use crate::router::{
    AttemptDisposition, AttemptEvidence, RoutedAttemptInput, RouterPolicy, RouterStopReason,
    classify_attempt,
};
use crate::task::TaskManagerHandle;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use uuid::Uuid;

const ROUTER_EVIDENCE_UPDATE_LIMIT: usize = 20;

pub async fn run_acp_router() -> io::Result<()> {
    let stdin = io::stdin();
    let mut lines = BufReader::new(stdin).lines();
    let mut stdout = io::stdout();
    let mut sessions = HashMap::new();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let request: Result<JsonRpcRequest, _> = serde_json::from_str(&line);
        match request {
            Ok(request) => {
                if let Some(response) =
                    handle_acp_router_request(request, &mut sessions, &mut stdout).await?
                {
                    write_response(&mut stdout, &response).await?;
                }
            }
            Err(_) => {
                let response = JsonRpcResponse::error(JsonRpcId::Null, -32700, "Parse error");
                write_response(&mut stdout, &response).await?;
            }
        }
    }
    Ok(())
}
```

Add this reusable execution API in the same file:

```rust
pub(crate) struct RouterPromptTurn {
    pub session_id: String,
    pub cwd: Option<String>,
    pub prompt: String,
    pub policy: RouterPolicy,
    pub mode: TaskMode,
    pub timeout_seconds: Option<i64>,
}

pub(crate) enum RouterUpdateSink<'a> {
    Acp { stdout: &'a mut io::Stdout },
    Silent,
}

pub(crate) struct RouterTerminalResult {
    pub stop_reason: &'static str,
    pub router_result: Value,
}

pub(crate) enum RouterPromptError {
    InvalidParams(String),
    Runtime(String),
}

impl RouterPromptError {
    pub(crate) fn into_json_rpc_response(self, id: JsonRpcId) -> JsonRpcResponse {
        match self {
            Self::InvalidParams(message) => JsonRpcResponse::error(id, -32602, message),
            Self::Runtime(message) => JsonRpcResponse::error(id, -32000, message),
        }
    }
}
```

- [ ] **Step 4: Refactor `run_acp_router_prompt` to call `execute_router_turn`**

In `router_runtime.rs`, keep ACP param parsing in `run_acp_router_prompt`, but make provider execution go through the shared function:

```rust
async fn run_acp_router_prompt(
    id: JsonRpcId,
    params: Option<Value>,
    sessions: &HashMap<String, RouterSession>,
    stdout: &mut io::Stdout,
) -> io::Result<Option<JsonRpcResponse>> {
    let params = match params.map(serde_json::from_value::<AcpPromptParams>) {
        Some(Ok(params)) => params,
        _ => {
            return Ok(Some(JsonRpcResponse::error(
                id,
                -32602,
                "invalid session/prompt params",
            )));
        }
    };
    let Some(session) = sessions.get(&params.session_id) else {
        return Ok(Some(JsonRpcResponse::error(
            id,
            -32602,
            "unknown router sessionId",
        )));
    };
    let Some(prompt) = acp_prompt_text(&params.prompt) else {
        return Ok(Some(JsonRpcResponse::error(
            id,
            -32602,
            "session/prompt requires text prompt content",
        )));
    };
    let candidates = params
        .policy
        .map(|policy| policy.candidates)
        .unwrap_or_else(|| vec![ProviderKind::Codex, ProviderKind::Claude]);
    let policy = match RouterPolicy::new(candidates) {
        Ok(policy) => policy,
        Err(error) => return Ok(Some(JsonRpcResponse::error(id, -32602, error.to_string()))),
    };
    let turn = RouterPromptTurn {
        session_id: params.session_id,
        cwd: session.cwd.clone(),
        prompt,
        policy,
        mode: params.mode.unwrap_or(TaskMode::Implement),
        timeout_seconds: params.timeout_seconds,
    };
    match execute_router_turn(turn, RouterUpdateSink::Acp { stdout }).await {
        Ok(result) => Ok(Some(JsonRpcResponse::result(
            id,
            json!({
                "stopReason": result.stop_reason,
                "routerResult": result.router_result
            }),
        ))),
        Err(error) => Ok(Some(error.into_json_rpc_response(id))),
    }
}
```

- [ ] **Step 5: Implement `execute_router_turn` with the existing loop and new result shape**

Move the current provider-attempt loop from `run_acp_router_prompt` into `execute_router_turn`. Its terminal success return must build this JSON shape:

```rust
        return Ok(RouterTerminalResult {
            stop_reason: response_stop_reason,
            router_result: json!({
                "provider": provider,
                "terminalKind": terminal_kind,
                "finalText": routed_final_text,
                "failureCategory": failure_category,
                "blockerReason": router_blocker_reason(disposition, stop_reason, failure_category),
                "verificationStatus": "not_verified",
                "attempts": attempts.clone(),
                "evidenceRefs": evidence_refs.clone(),
                "diagnostics": {
                    "provider": provider,
                    "terminalKind": terminal_kind,
                    "attempts": attempts,
                    "failoverTrail": failover_trail,
                    "evidenceRefs": evidence_refs,
                    "bounded": true
                }
            }),
        });
```

Where the old code emitted ACP notifications directly, gate them on the sink:

```rust
        if let Some(transcript) = transcript.as_ref() {
            if let RouterUpdateSink::Acp { stdout } = &mut updates {
                write_router_evidence_updates(stdout, &turn.session_id, provider, transcript)
                    .await?;
            }
        }
```

And for final text chunks:

```rust
        if let Some(text) = routed_final_text.as_deref()
            && let RouterUpdateSink::Acp { stdout } = &mut updates
        {
            let notification = JsonRpcNotification::new(
                "session/update",
                json!({
                    "sessionId": turn.session_id,
                    "update": {
                        "sessionUpdate": "agent_message_chunk",
                        "content": {"type": "text", "text": text}
                    }
                }),
            );
            write_json_message(stdout, &notification).await?;
        }
```

If no provider attempts run, return:

```rust
    Err(RouterPromptError::Runtime(
        "router policy did not produce an attempt".to_string(),
    ))
```

- [ ] **Step 6: Update module exports and runtime imports**

In `crates/agent-bridge-mcp/src/lib.rs`, add:

```rust
pub mod router_runtime;
```

In `crates/agent-bridge-mcp/src/runtime.rs`, remove ACP-router-specific imports and change default dispatch to:

```rust
crate::router_runtime::run_acp_router().await
```

- [ ] **Step 7: Run focused ACP router tests**

Run:

```bash
cargo test -p agent-bridge-mcp --test stdio_binary stdio_binary_defaults_to_acp_router_without_starting_mcp_loop stdio_acp_router_prompt_runs_one_provider_turn stdio_acp_router_prompt_fails_over_after_infrastructure_failure stdio_acp_router_prompt_returns_blocker_for_refusal_stop_reason -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/agent-bridge-mcp/src/runtime.rs crates/agent-bridge-mcp/src/router_runtime.rs crates/agent-bridge-mcp/src/lib.rs crates/agent-bridge-mcp/tests/stdio_binary.rs
git commit -m "refactor: share acp router turn execution"
```

---

### Task 3: Add Minimal MCP Adapter With `agent_delegate`

**Files:**
- Create: `crates/agent-bridge-mcp/src/mcp_adapter.rs`
- Modify: `crates/agent-bridge-mcp/src/runtime.rs`
- Modify: `crates/agent-bridge-mcp/src/lib.rs`
- Modify: `crates/agent-bridge-mcp/tests/stdio_binary.rs`
- Create: `crates/agent-bridge-mcp/tests/mcp_adapter_protocol.rs`

**Interfaces:**
- Consumes: `router_runtime::execute_router_turn`, `RouterPromptTurn`, `RouterUpdateSink`, `RouterPromptError`.
- Produces:
  - `pub async fn run() -> tokio::io::Result<()>`
  - MCP adapter `tools/list` returns exactly `agent_delegate` until Task 4 adds evidence reads.
  - MCP adapter `agent_delegate` waits for one router terminal result and returns the router result body as MCP text content.

- [ ] **Step 1: Add adapter command helper and delegate-only tool-list test**

In `crates/agent-bridge-mcp/tests/stdio_binary.rs`, add:

```rust
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
```

Add:

```rust
#[test]
fn mcp_adapter_lists_only_delegate_tool_before_evidence_task() {
    let env = FixtureEnv::new("mcp-adapter-tool-list");
    let mut client = McpClient::start_with_command(mcp_adapter_command(&env));
    let response = client.request("tools/list", json!({}));
    let tools = response["result"]["tools"].as_array().unwrap();
    let names: Vec<_> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();

    assert_eq!(names, vec!["agent_delegate"]);
    for removed in [
        "providers_list",
        "doctor",
        "agent_spawn",
        "agent_list",
        "agent_observe",
        "agent_result",
        "agent_stop",
        "agent_remove",
    ] {
        assert!(!names.contains(&removed), "adapter exposed {removed}");
    }
}
```

Add this helper constructor to `impl McpClient`:

```rust
    fn start_with_command(mut command: Command) -> Self {
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
```

- [ ] **Step 2: Add adapter `agent_delegate` stdio test**

In `crates/agent-bridge-mcp/tests/stdio_binary.rs`, add:

```rust
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
    assert!(result["finalText"].as_str().unwrap().contains("fake provider"));
    assert!(result["evidenceRefs"].as_array().unwrap().len() >= 1);
    assert_eq!(
        result["diagnostics"]["evidenceRefs"],
        result["evidenceRefs"]
    );
}
```

- [ ] **Step 3: Add protocol-level adapter tests**

Create `crates/agent-bridge-mcp/tests/mcp_adapter_protocol.rs`:

```rust
use agent_bridge_mcp::mcp::{JsonRpcId, JsonRpcRequest};
use agent_bridge_mcp::mcp_adapter::handle_request_for_test;
use serde_json::{Value, json};

fn request(method: &str, id: i64, params: Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(JsonRpcId::Number(id)),
        method: method.to_string(),
        params: Some(params),
    }
}

#[tokio::test]
async fn initialize_advertises_minimal_mcp_adapter() {
    let response = handle_request_for_test(request("initialize", 1, json!({})))
        .await
        .unwrap();
    let result = response.result.unwrap();

    assert_eq!(result["protocolVersion"], "2024-11-05");
    assert_eq!(result["capabilities"], json!({"tools": {}}));
    assert_eq!(result["serverInfo"]["name"], "agent-bridge-mcp-adapter");
    assert!(
        result["instructions"]
            .as_str()
            .unwrap()
            .contains("agent_delegate")
    );
}

#[tokio::test]
async fn tools_list_contains_delegate_and_no_lifecycle_tools() {
    let response = handle_request_for_test(request("tools/list", 2, json!({})))
        .await
        .unwrap();
    let tools = response.result.unwrap()["tools"].as_array().unwrap().clone();
    let names: Vec<_> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();

    assert_eq!(names, vec!["agent_delegate"]);
    assert!(!names.contains(&"agent_spawn"));
    assert!(!names.contains(&"agent_observe"));
    assert!(!names.contains(&"agent_result"));
}
```

- [ ] **Step 4: Run the failing adapter tests**

Run:

```bash
cargo test -p agent-bridge-mcp --test stdio_binary mcp_adapter_lists_only_delegate_tool_before_evidence_task mcp_adapter_agent_delegate_returns_router_result -- --test-threads=1
cargo test -p agent-bridge-mcp --test mcp_adapter_protocol
```

Expected: FAIL because `mcp_adapter` is not implemented yet.

- [ ] **Step 5: Create `mcp_adapter.rs`**

Create `crates/agent-bridge-mcp/src/mcp_adapter.rs`:

```rust
use crate::domain::{ProviderKind, TaskMode};
use crate::mcp::{JsonRpcId, JsonRpcRequest, JsonRpcResponse};
use crate::router::RouterPolicy;
use crate::router_runtime::{
    RouterPromptError, RouterPromptTurn, RouterUpdateSink, execute_router_turn,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use uuid::Uuid;

const PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum AdapterToolName {
    #[serde(rename = "agent_delegate")]
    AgentDelegate,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdapterToolCallParams {
    name: AdapterToolName,
    #[serde(default)]
    arguments: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DelegateArguments {
    prompt: String,
    cwd: Option<String>,
    mode: Option<TaskMode>,
    timeout_seconds: Option<i64>,
    policy: Option<DelegatePolicy>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DelegatePolicy {
    candidates: Vec<ProviderKind>,
}

pub async fn run() -> io::Result<()> {
    let stdin = io::stdin();
    let mut lines = BufReader::new(stdin).lines();
    let mut stdout = io::stdout();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(request) => handle_request(request).await,
            Err(_) => Some(JsonRpcResponse::error(JsonRpcId::Null, -32700, "Parse error")),
        };
        if let Some(response) = response {
            write_response(&mut stdout, &response).await?;
        }
    }
    Ok(())
}

#[doc(hidden)]
pub async fn handle_request_for_test(request: JsonRpcRequest) -> Option<JsonRpcResponse> {
    handle_request(request).await
}

async fn handle_request(request: JsonRpcRequest) -> Option<JsonRpcResponse> {
    let id = request.id?;
    let response = match request.method.as_str() {
        "initialize" => JsonRpcResponse::result(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "agent-bridge-mcp-adapter", "version": "0.1.0"},
                "instructions": "Use agent_delegate for one routed provider turn. Provider output is evidence; caller verification remains required."
            }),
        ),
        "tools/list" => JsonRpcResponse::result(id, json!({"tools": adapter_tool_definitions()})),
        "tools/call" => JsonRpcResponse::result(
            id,
            call_tool(request.params.unwrap_or_else(|| json!({}))).await,
        ),
        _ => JsonRpcResponse::error(id, -32601, format!("Method not found: {}", request.method)),
    };
    Some(response)
}

async fn call_tool(params: Value) -> Value {
    let parsed = match serde_json::from_value::<AdapterToolCallParams>(params) {
        Ok(parsed) => parsed,
        Err(error) => return tool_error(error.to_string()),
    };
    match parsed.name {
        AdapterToolName::AgentDelegate => match agent_delegate(parsed.arguments).await {
            Ok(result) => tool_json(result),
            Err(error) => tool_error(error),
        },
    }
}

async fn agent_delegate(arguments: Value) -> Result<Value, String> {
    let arguments: DelegateArguments =
        serde_json::from_value(arguments).map_err(|error| error.to_string())?;
    let candidates = arguments
        .policy
        .map(|policy| policy.candidates)
        .unwrap_or_else(|| vec![ProviderKind::Codex, ProviderKind::Claude]);
    let policy = RouterPolicy::new(candidates).map_err(|error| error.to_string())?;
    let turn = RouterPromptTurn {
        session_id: format!("mcp-adapter-{}", Uuid::new_v4().simple()),
        cwd: arguments.cwd,
        prompt: arguments.prompt,
        policy,
        mode: arguments.mode.unwrap_or(TaskMode::Implement),
        timeout_seconds: arguments.timeout_seconds,
    };
    execute_router_turn(turn, RouterUpdateSink::Silent)
        .await
        .map(|result| result.router_result)
        .map_err(router_prompt_error_text)
}

fn router_prompt_error_text(error: RouterPromptError) -> String {
    match error {
        RouterPromptError::InvalidParams(message) | RouterPromptError::Runtime(message) => message,
    }
}

fn adapter_tool_definitions() -> Vec<Value> {
    vec![json!({
            "name": "agent_delegate",
            "description": "Run one routed provider turn and return a terminal router result. The caller remains responsible for verification.",
            "inputSchema": object_schema(
                json!({
                    "prompt": {"type": "string"},
                    "cwd": {"type": "string"},
                    "mode": {"type": "string", "enum": ["review", "implement", "command"]},
                    "timeoutSeconds": {"type": "integer", "minimum": 1, "maximum": 1800},
                    "policy": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "candidates": {
                                "type": "array",
                                "items": {"type": "string", "enum": ["codex", "claude", "cursor", "kimi", "pi", "forge", "antigravity"]}
                            }
                        },
                        "required": ["candidates"]
                    }
                }),
                vec!["prompt"]
            ),
            "annotations": {
                "title": "Delegate to agent",
                "readOnlyHint": false,
                "destructiveHint": false,
                "idempotentHint": false,
                "openWorldHint": true
            }
        })]
}

fn object_schema(properties: Value, required: Vec<&str>) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": properties
    })
}

fn tool_json(value: Value) -> Value {
    json!({
        "content": [{"type": "text", "text": serde_json::to_string(&value).unwrap()}],
        "isError": false
    })
}

fn tool_error(message: String) -> Value {
    json!({
        "content": [{"type": "text", "text": message}],
        "isError": true
    })
}

async fn write_response(stdout: &mut io::Stdout, response: &JsonRpcResponse) -> io::Result<()> {
    let mut line = serde_json::to_vec(response).map_err(io::Error::other)?;
    line.push(b'\n');
    stdout.write_all(&line).await?;
    stdout.flush().await
}
```

- [ ] **Step 6: Wire the module and subcommand**

In `crates/agent-bridge-mcp/src/lib.rs`, add:

```rust
pub mod mcp_adapter;
```

In `runtime.rs`, ensure `CliCommand::McpAdapter` dispatches to:

```rust
#[derive(Subcommand, Debug)]
enum CliCommand {
    /// Ask a running Agent Bridge server to reload config from disk.
    Reload,
    /// Run the minimal MCP adapter for hosts that cannot launch ACP agents directly.
    McpAdapter,
    /// Run the Agent Bridge-owned Claude host runner on the given Unix socket.
    ClaudeHostRunner { socket: std::path::PathBuf },
}
```

And add this match arm before `ClaudeHostRunner`:

```rust
        Some(CliCommand::McpAdapter) => {
            if let Err(error) = crate::mcp_adapter::run().await {
                tracing::error!(error = %error, "[agent-bridge] fatal {error}");
                std::process::exit(1);
            }
            return;
        }
```

The call inside the arm is:

```rust
crate::mcp_adapter::run().await
```

- [ ] **Step 7: Run adapter tests**

Run:

```bash
cargo test -p agent-bridge-mcp --test stdio_binary mcp_adapter_lists_only_delegate_tool_before_evidence_task mcp_adapter_agent_delegate_returns_router_result -- --test-threads=1
cargo test -p agent-bridge-mcp --test mcp_adapter_protocol
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/agent-bridge-mcp/src/mcp_adapter.rs crates/agent-bridge-mcp/src/runtime.rs crates/agent-bridge-mcp/src/lib.rs crates/agent-bridge-mcp/tests/stdio_binary.rs crates/agent-bridge-mcp/tests/mcp_adapter_protocol.rs
git commit -m "feat: add minimal mcp router adapter"
```

---

### Task 4: Implement Adapter `agent_evidence`

**Files:**
- Modify: `crates/agent-bridge-mcp/src/mcp_adapter.rs`
- Modify: `crates/agent-bridge-mcp/tests/stdio_binary.rs`
- Modify: `crates/agent-bridge-mcp/tests/mcp_adapter_protocol.rs`

**Interfaces:**
- Consumes: `TaskManagerHandle::result(agent_id, ResultSections, max_bytes, stdout_line, stderr_line, cursor, limit, detailed)`.
- Produces: `agent_evidence` returns bounded evidence for an `evidenceRef` object containing `agentId`.

- [ ] **Step 1: Update stdio tool-list coverage and add `agent_evidence` coverage**

In `crates/agent-bridge-mcp/tests/stdio_binary.rs`, rename `mcp_adapter_lists_only_delegate_tool_before_evidence_task` to `mcp_adapter_lists_only_delegate_and_evidence_tools` and change the expected names:

```rust
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
    for removed in [
        "providers_list",
        "doctor",
        "agent_spawn",
        "agent_list",
        "agent_observe",
        "agent_result",
        "agent_stop",
        "agent_remove",
    ] {
        assert!(!names.contains(&removed), "adapter exposed {removed}");
    }
}
```

Then add:

```rust
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
```

- [ ] **Step 2: Update protocol coverage for final tool list and input schema**

In `mcp_adapter_protocol.rs`, replace `tools_list_contains_delegate_and_no_lifecycle_tools` with:

```rust
#[tokio::test]
async fn tools_list_contains_delegate_evidence_and_no_lifecycle_tools() {
    let response = handle_request_for_test(request("tools/list", 2, json!({})))
        .await
        .unwrap();
    let tools = response.result.unwrap()["tools"].as_array().unwrap().clone();
    let names: Vec<_> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();

    assert_eq!(names, vec!["agent_delegate", "agent_evidence"]);
    assert!(!names.contains(&"agent_spawn"));
    assert!(!names.contains(&"agent_observe"));
    assert!(!names.contains(&"agent_result"));
}
```

Then add:

```rust
#[tokio::test]
async fn evidence_schema_requires_evidence_ref_only() {
    let response = handle_request_for_test(request("tools/list", 3, json!({})))
        .await
        .unwrap();
    let tools = response.result.unwrap()["tools"].as_array().unwrap().clone();
    let evidence = tools
        .iter()
        .find(|tool| tool["name"] == "agent_evidence")
        .expect("agent_evidence tool");

    assert_eq!(evidence["inputSchema"]["required"], json!(["evidenceRef"]));
    assert_eq!(
        evidence["inputSchema"]["properties"]["sections"]["items"]["enum"],
        json!(["summary", "stdout", "stderr", "transcript", "diff", "changedFiles"])
    );
}
```

- [ ] **Step 3: Run the failing evidence tests**

Run:

```bash
cargo test -p agent-bridge-mcp --test stdio_binary mcp_adapter_lists_only_delegate_and_evidence_tools mcp_adapter_agent_evidence_reads_bounded_sections -- --test-threads=1
cargo test -p agent-bridge-mcp --test mcp_adapter_protocol evidence_schema_requires_evidence_ref_only
```

Expected: FAIL because `agent_evidence` is not listed or implemented yet.

- [ ] **Step 4: Add evidence argument parsing**

In `mcp_adapter.rs`, add:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EvidenceArguments {
    evidence_ref: EvidenceRefArgument,
    #[serde(default)]
    sections: Vec<String>,
    max_bytes: Option<i64>,
    stdout_line: Option<u64>,
    stderr_line: Option<u64>,
    cursor: Option<u64>,
    limit: Option<u64>,
    verbosity: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EvidenceRefArgument {
    agent_id: String,
}
```

- [ ] **Step 5: Add the `AgentEvidence` tool name, schema, and dispatch**

In `AdapterToolName`, add:

```rust
    #[serde(rename = "agent_evidence")]
    AgentEvidence,
```

In `call_tool`, add this branch:

```rust
        AdapterToolName::AgentEvidence => match agent_evidence(parsed.arguments).await {
            Ok(result) => tool_json(result),
            Err(error) => tool_error(error),
        },
```

In `adapter_tool_definitions()`, change `vec![json!({ ... agent_delegate ... })]` to a two-item vector by appending:

```rust
        json!({
            "name": "agent_evidence",
            "description": "Fetch bounded evidence for an evidenceRef returned by agent_delegate.",
            "inputSchema": object_schema(
                json!({
                    "evidenceRef": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {"agentId": {"type": "string"}},
                        "required": ["agentId"]
                    },
                    "sections": {
                        "type": "array",
                        "items": {"type": "string", "enum": ["summary", "stdout", "stderr", "transcript", "diff", "changedFiles"]}
                    },
                    "maxBytes": {"type": "integer", "minimum": 1},
                    "stdoutLine": {"type": "integer", "minimum": 0},
                    "stderrLine": {"type": "integer", "minimum": 0},
                    "cursor": {"type": "integer", "minimum": 0},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 500},
                    "verbosity": {"type": "string", "enum": ["compact", "detailed"]}
                }),
                vec!["evidenceRef"]
            ),
            "annotations": {
                "title": "Read agent evidence",
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false
            }
        })
```

Add:

```rust
async fn agent_evidence(arguments: Value) -> Result<Value, String> {
    let arguments: EvidenceArguments =
        serde_json::from_value(arguments).map_err(|error| error.to_string())?;
    let sections = if arguments.sections.is_empty() {
        crate::task::ResultSections::default_sections()
    } else {
        crate::task::ResultSections::from_names(arguments.sections.iter().map(String::as_str))
    };
    let stdout_line = arguments.stdout_line.map(|value| value as usize);
    let stderr_line = arguments.stderr_line.map(|value| value as usize);
    let detailed = arguments.verbosity.as_deref() == Some("detailed");
    let manager = crate::task::TaskManagerHandle::from_env().await?;
    manager
        .result(
            arguments.evidence_ref.agent_id,
            sections,
            arguments.max_bytes,
            stdout_line,
            stderr_line,
            arguments.cursor,
            arguments.limit,
            detailed,
        )
        .await
}
```

- [ ] **Step 6: Run evidence tests**

Run:

```bash
cargo test -p agent-bridge-mcp --test stdio_binary mcp_adapter_lists_only_delegate_and_evidence_tools mcp_adapter_agent_evidence_reads_bounded_sections -- --test-threads=1
cargo test -p agent-bridge-mcp --test mcp_adapter_protocol
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/agent-bridge-mcp/src/mcp_adapter.rs crates/agent-bridge-mcp/tests/stdio_binary.rs crates/agent-bridge-mcp/tests/mcp_adapter_protocol.rs
git commit -m "feat: expose bounded adapter evidence"
```

---

### Task 5: Remove Old MCP Lifecycle Modules And Tests

**Files:**
- Modify: `crates/agent-bridge-mcp/src/lib.rs`
- Modify: `crates/agent-bridge-mcp/src/server.rs`
- Delete: `crates/agent-bridge-mcp/src/guidance.rs`
- Delete: `crates/agent-bridge-mcp/src/tools.rs`
- Create: `crates/agent-bridge-mcp/src/task/input.rs`
- Modify: `crates/agent-bridge-mcp/src/task.rs`
- Modify: `crates/agent-bridge-mcp/src/task/spawn.rs`
- Modify: `crates/agent-bridge-mcp/tests/protocol_models.rs`
- Delete: `crates/agent-bridge-mcp/tests/server_protocol.rs`
- Modify: `crates/agent-bridge-mcp/tests/stdio_binary.rs`

**Interfaces:**
- Consumes: adapter tests from Tasks 3 and 4.
- Produces:
  - No public old lifecycle MCP module exports.
  - No old lifecycle tool definitions in tests or docs.
  - `TaskPreviewInput` lives in `task::input`.
  - Internal doctor/diagnostics still supports `--doctor-smoke`.

- [ ] **Step 1: Move `TaskPreviewInput` out of `tools.rs`**

Create `crates/agent-bridge-mcp/src/task/input.rs`:

```rust
use crate::domain::{Isolation, LaunchProfile, ProviderKind, RetryPolicy, TaskMode};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskPreviewInput {
    pub provider: ProviderKind,
    pub mode: TaskMode,
    pub prompt: String,
    pub title: Option<String>,
    pub cwd: Option<String>,
    pub timeout_seconds: Option<i64>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub thinking: Option<String>,
    pub isolation: Option<Isolation>,
    pub worktree_name: Option<String>,
    pub profile: Option<LaunchProfile>,
    #[serde(default)]
    pub dry_run: Option<bool>,
    #[serde(default)]
    pub retry_policy: Option<RetryPolicy>,
}
```

In `crates/agent-bridge-mcp/src/task.rs`, add the module and re-export:

```rust
pub mod input;
pub use input::TaskPreviewInput;
```

In `crates/agent-bridge-mcp/src/task/spawn.rs`, replace:

```rust
use crate::tools::TaskPreviewInput;
```

with:

```rust
use super::input::TaskPreviewInput;
```

- [ ] **Step 2: Update protocol model tests away from old tool names**

In `crates/agent-bridge-mcp/tests/protocol_models.rs`, replace:

```rust
use agent_bridge_mcp::tools::{TaskPreviewInput, ToolCallParams, ToolName};
```

with:

```rust
use agent_bridge_mcp::task::TaskPreviewInput;
```

Delete the `tool_call_params_parse_known_tool_names` test. Keep JSON-RPC and domain tests unchanged.

- [ ] **Step 3: Delete lifecycle MCP protocol tests**

Delete `crates/agent-bridge-mcp/tests/server_protocol.rs`. The replacement coverage is `mcp_adapter_protocol.rs`.

- [ ] **Step 4: Remove lifecycle-focused stdio tests**

In `crates/agent-bridge-mcp/tests/stdio_binary.rs`, remove tests and helper branches that call old MCP tools directly through `McpClient::start`, including calls to:

```text
providers_list
doctor
agent_spawn
agent_list
agent_observe
agent_result
agent_stop
agent_remove
prompts/list
prompts/get
resources/list
resources/read
```

Keep tests that exercise:

```text
--config-check
--doctor-smoke
panic hook cleanup
default ACP initialize/session/new/session/prompt
ACP failover/blocker/failure behavior
mcp-adapter tools/list
mcp-adapter agent_delegate
mcp-adapter agent_evidence
claude-host-runner behavior
task manager unit behavior not tied to MCP tool names
```

- [ ] **Step 5: Slim `server.rs` to diagnostics only**

In `crates/agent-bridge-mcp/src/server.rs`, remove:

```rust
use crate::guidance;
use crate::mcp::{JsonRpcId, JsonRpcRequest, JsonRpcResponse};
use crate::tools::{TaskPreviewInput, ToolCallParams, ToolName, tool_definitions};
```

Remove these old MCP functions:

```rust
pub async fn handle_request(...)
async fn call_tool(...)
async fn handle_agent_observe(...)
async fn handle_agent_result(...)
fn is_detailed(...)
fn result_sections(...)
fn reject_unknown_arguments(...)
fn tool_name_str(...)
async fn agent_list(...)
fn agent_list_arguments(...)
fn agent_list_response(...)
fn require_agent_id(...)
fn task_preview(...)
fn preview_command(...)
fn tool_result(...)
fn tool_json(...)
fn tool_error(...)
```

Keep `pub async fn doctor_report(arguments: Value) -> Result<Value, String>` and the imports/constants/helpers used by `mod diagnostics`. If compiler errors identify an import used only by removed MCP handlers, remove that import in the same edit.

- [ ] **Step 6: Delete old MCP guidance/tool modules and stop public exports**

Delete:

```text
crates/agent-bridge-mcp/src/guidance.rs
crates/agent-bridge-mcp/src/tools.rs
```

In `crates/agent-bridge-mcp/src/lib.rs`, replace the module list with:

```rust
#![deny(clippy::print_stderr, clippy::print_stdout)]

pub mod claude_host;
pub mod claude_interactive;
pub mod config;
pub mod domain;
pub mod mcp;
pub mod mcp_adapter;
pub mod provider;
pub mod router;
pub mod router_runtime;
pub mod runtime;
mod server;
pub mod task;
```

- [ ] **Step 7: Run targeted compile and tests**

Run:

```bash
cargo test -p agent-bridge-mcp --test protocol_models
cargo test -p agent-bridge-mcp --test mcp_adapter_protocol
cargo test -p agent-bridge-mcp --test stdio_binary stdio_binary_defaults_to_acp_router_without_starting_mcp_loop mcp_adapter_lists_only_delegate_and_evidence_tools mcp_adapter_agent_delegate_returns_router_result mcp_adapter_agent_evidence_reads_bounded_sections -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 8: Verify old lifecycle names are gone from source and tests**

Run:

```bash
rg -n "agent_spawn|agent_observe|agent_result|agent_list|agent_stop|agent_remove|providers_list|prompts/list|resources/list" crates/agent-bridge-mcp/src crates/agent-bridge-mcp/tests
```

Expected: no matches, except historical text in committed spec/plan files outside `src` and `tests`.

- [ ] **Step 9: Commit**

```bash
git add -A crates/agent-bridge-mcp/src crates/agent-bridge-mcp/tests
git commit -m "refactor: remove legacy mcp lifecycle surface"
```

---

### Task 6: Update Docs And Run Full Validation

**Files:**
- Modify: `README.md`
- Modify: `docs/agents/architecture.md`
- Modify: `docs/agents/definition-of-done.md` only if it names old MCP lifecycle commands as the default usage path.

**Interfaces:**
- Consumes: final runtime behavior from Tasks 1-5.
- Produces: docs that describe ACP default, `mcp-adapter`, `agent_delegate`, and `agent_evidence`.

- [ ] **Step 1: Update README runtime section**

Replace old MCP lifecycle usage text with:

```markdown
## Runtime Modes

`agent-bridge-mcp` runs the ACP router by default over newline-delimited JSON-RPC.

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
{"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"/repo"}}
{"jsonrpc":"2.0","id":3,"method":"session/prompt","params":{"sessionId":"router-...","mode":"implement","prompt":{"type":"text","text":"Fix the failing test"},"timeoutSeconds":600}}
```

Hosts that can only integrate external tools through MCP can run:

```bash
agent-bridge-mcp mcp-adapter
```

The adapter exposes two tools:

- `agent_delegate`: run one routed provider turn and return a terminal router result.
- `agent_evidence`: fetch bounded evidence for an evidence reference returned by `agent_delegate`.

The router result always includes `verificationStatus: "not_verified"`. Provider output is evidence; the caller remains responsible for local verification.
```

- [ ] **Step 2: Update architecture docs**

In `docs/agents/architecture.md`, replace any section that says the public contract is the eight MCP tools with:

```markdown
## Public Protocol Surface

Agent Bridge is ACP-router-first.

- Default command: `agent-bridge-mcp`
- Default protocol: newline-delimited ACP-flavored JSON-RPC
- Supported default methods: `initialize`, `session/new`, `session/prompt`
- Terminal result kinds: `answer`, `blocker`, `failure`
- Verification status: always `not_verified`

The MCP surface is a small adapter for clients that cannot launch ACP agents directly:

- Command: `agent-bridge-mcp mcp-adapter`
- Tools: `agent_delegate`, `agent_evidence`

The task manager, provider adapters, transcript capture, worktree isolation, and result section readers remain internal implementation details.
```

- [ ] **Step 3: Run source/docs search for removed public lifecycle contract**

Run:

```bash
rg -n "eight-tool|agent_spawn|agent_observe|agent_result|agent_list|agent_stop|agent_remove|providers_list" README.md docs crates/agent-bridge-mcp/src crates/agent-bridge-mcp/tests
```

Expected: no current-contract matches. Matches inside historical design/spec/plan docs are allowed only when they explicitly say the lifecycle surface was removed.

- [ ] **Step 4: Run full crate tests**

Run:

```bash
cargo test -p agent-bridge-mcp -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 5: Run full quality gate**

Run:

```bash
scripts/quality.sh
```

Expected: PASS with rustfmt, clippy `-D warnings`, cargo-machete, tests, and jscpd below the configured threshold.

- [ ] **Step 6: Commit docs and any validation fallout fixes**

```bash
git add README.md docs/agents/architecture.md docs/agents/definition-of-done.md crates/agent-bridge-mcp
git commit -m "docs: document acp default and mcp adapter"
```

---

## Self-Review Checklist For Implementers

- Default `agent-bridge-mcp` accepts ACP `initialize`, `session/new`, and `session/prompt`.
- Default `agent-bridge-mcp` rejects MCP `tools/list` with `-32601`.
- `agent-bridge-mcp mcp-adapter` lists exactly `agent_delegate` and `agent_evidence`.
- `agent_delegate` returns one terminal router result with `verificationStatus: "not_verified"`.
- `agent_evidence` fetches bounded evidence using existing task manager result readers.
- Old MCP lifecycle tool names are not present in `crates/agent-bridge-mcp/src` or active tests.
- Diagnostics remain available through `--doctor-smoke`.
- No new dependency was added.
- Full `scripts/quality.sh` passes before the branch is considered ready.
