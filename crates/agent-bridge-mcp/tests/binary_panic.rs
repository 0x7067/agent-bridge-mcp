use std::process::{Command, Stdio};

#[test]
fn forced_panic_writes_stderr_without_stdout_and_exits_nonzero() {
    let output = Command::new(env!("CARGO_BIN_EXE_agent-bridge-mcp"))
        .env("AGENT_BRIDGE_FORCE_PANIC", "1")
        .stdin(Stdio::null())
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("[agent-bridge] panic"));
    assert!(stderr.contains("forced panic for integration test"));
}
