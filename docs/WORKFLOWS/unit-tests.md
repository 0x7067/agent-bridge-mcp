# Testing Workflows

**Last Updated:** 2026-06-07
**Based on patterns from:** `tests/protocol_models.rs`, `tests/server_protocol.rs`, `tests/binary_panic.rs`, `tests/claude_interactive_runner.rs`

## How to Write a Deterministic Fake-Provider Test

Agent Bridge deliberately avoids requiring paid API keys, internet access, or real provider CLIs in its default test suite. Use fake scripts and deterministic assertions.

### Pattern: Fixture Script + Assert

Place a fake provider script in `tests/fixtures/my_provider/fake.sh`:

```bash
#!/usr/bin/env bash
# Simulates a provider that prints a marker and exits 0.
echo "AGENT_BRIDGE_PROVIDER_SMOKE_OK"
```

Make it executable:

```bash
chmod +x tests/fixtures/my_provider/fake.sh
```

In a Rust test, temporarily manipulate PATH or provider lookup to invoke the fake:

```rust
// tests/server_protocol.rs
use std::env;

#[test]
fn my_provider_smoke_ok() {
    // Arrange: point the test at the fake script
    let fake_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/my_provider");
    let original_path = env::var_os("PATH");
    env::set_var("PATH", format!("{}:{} {}", fake_dir, original_path.as_ref().unwrap()));

    // Act: call doctor with smoke targeting the provider
    let response = call_doctor_sync(json!({
        "focus": "providers",
        "smoke": true,
        "providers": ["my_provider"]
    }));

    // Assert
    assert_eq!(response["providers"]["my_provider"]["readiness"]["state"], "ready");

    // Cleanup
    if let Some(p) = original_path { env::set_var("PATH", p); }
}
```

### Pattern: Protocol Model Roundtrip

For anything touching `JsonRpcRequest`/`JsonRpcResponse` or tool param parsing, add a model test in `tests/protocol_models.rs`:

```rust
// tests/protocol_models.rs
#[test]
fn my_new_tool_params_deserialize_correctly() {
    let input = r#"{"requiredField":"hi","optionalFlag":true}"#;
    let parsed: MyNewToolInput = serde_json::from_str(input).unwrap();
    assert_eq!(parsed.required_field, "hi");
    assert!(parsed.optional_flag.unwrap());
}
```

## How to Write a PTY-Sensitive Test

Tests that spawn real child processes or allocate PTYs must run single-threaded to avoid cross-kill flakiness:

```bash
cargo test --test pty_adapter_spike -- --test-threads=1
```

Inside the test, use the isolated `ActivePids` constructor (not the global `ACTIVE_PIDS`):

```rust
// tests/pty_adapter_spike.rs
use agent_bridge_mcp::task::supervision::ActivePids;

#[tokio::test]
async fn my_custom_kill_scenario() {
    let registry = ActivePids::new(); // isolated
    let mut child = spawn_test_child();
    let pid = child.id().unwrap();
    registry.register(pid);
    registry.terminate_all(libc::SIGTERM);
    let status = timeout(Duration::from_secs(3), child.wait()).await.unwrap().unwrap();
    assert_eq!(signal_name(&status).as_deref(), Some("SIGTERM"));
}
```

## How to Write a Binary / Panic Recovery Test

Binary tests in `tests/stdio_binary.rs` exercise the actual compiled binary. Typical pattern:

```rust
// tests/binary_panic.rs
use std::process::{Command, Stdio};

#[test]
fn forced_panic_kills_children() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_agent-bridge-mcp"))
        .env("AGENT_BRIDGE_FORCE_PANIC", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let status = child.wait().unwrap();
    // Expect abnormal exit due to panic hook
    assert!(!status.success());
}
```

## How to Write a Server Protocol Test

Use the synchronous helpers in `tests/server_protocol.rs`:

```rust
fn call_tool_sync(method: &str, arguments: Value) -> Value {
    // ...spawns the server, sends ND-JSON, reads response...
}
```

Assert on the resulting JSON. Avoid asserting on unstable timestamps or UUIDs; assert presence/absence of keys and status enumerations.

### Checklist

- [ ] Fake script or deterministic fixture used where possible
- [ ] Real child processes guarded with `--test-threads=1` or isolated `ActivePids`
- [ ] Assertions avoid hardcoded timestamps or random UUIDs
- [ ] Test cleans up temp files/directories in `Drop` or `finally` equivalents
- [ ] `./scripts/quality.sh` passes before pushing
