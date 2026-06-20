# Testing Workflows

**Last Updated:** 2026-06-20
**Based on patterns from:** `tests/protocol_models.rs`, `tests/mcp_adapter_protocol.rs`, `tests/stdio_binary.rs`, `tests/binary_panic.rs`, `tests/claude_interactive_runner.rs`

## How to Write a Deterministic Fake-Provider Test

Agent Bridge deliberately avoids requiring paid API keys, internet access, or real provider CLIs in its default test suite. Use fake scripts and deterministic assertions.

### Pattern: Fixture Script + Assert

Place a fake provider script in `tests/fixtures/my_provider/fake.sh`:

```bash
#!/usr/bin/env bash
# Simulates a provider that prints a final result and exits 0.
echo "provider smoke ok"
```

Make it executable:

```bash
chmod +x tests/fixtures/my_provider/fake.sh
```

In a Rust test, temporarily point the relevant provider binary environment
variable at the fake. Prefer the existing env guard helpers in the test suite so
global process environment is restored even when the assertion fails:

```rust
#[test]
fn my_provider_smoke_ok() {
    let _env = EnvGuard::set("CODEX_ACP_BIN", fake_provider_path());
    // Exercise the public surface that should launch the provider.
    // Prefer stdio_binary.rs for full stdio behavior or
    // mcp_adapter_protocol.rs for adapter-only behavior.
}
```

### Pattern: Protocol Model Roundtrip

For anything touching `JsonRpcRequest`/`JsonRpcResponse` or adapter input
parsing, add a model or adapter test:

```rust
#[test]
fn adapter_input_contains_required_fields() {
    let value: serde_json::Value =
        serde_json::from_str(r#"{"prompt":"hi","provider":"codex","mode":"research"}"#).unwrap();
    assert_eq!(value["prompt"], "hi");
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

## How to Write a Stdio Protocol Test

Use the synchronous helpers in `tests/stdio_binary.rs` or
`tests/mcp_adapter_protocol.rs`:

```rust
fn call_adapter_tool(name: &str, arguments: Value) -> Value {
    // ...spawns the server, sends ND-JSON, reads response...
}
```

Assert on the resulting JSON. Avoid unstable timestamps or UUIDs; assert
presence/absence of keys and status enumerations.

### Checklist

- [ ] Fake script or deterministic fixture used where possible
- [ ] Real child processes guarded with `--test-threads=1` or isolated `ActivePids`
- [ ] Assertions avoid hardcoded timestamps or random UUIDs
- [ ] Test cleans up temp files/directories in `Drop` or `finally` equivalents
- [ ] `./scripts/quality.sh` passes before pushing
