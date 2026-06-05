use agent_bridge_mcp::claude_interactive::runner::{ClaudeRunnerRequest, spawn_claude};
use agent_bridge_mcp::domain::TaskMode;
use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::time::timeout;

const FIXTURE_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/interactive_claude"
);

#[tokio::test]
async fn claude_runner_spawns_interactive_binary_through_login_shell_pty() -> io::Result<()> {
    let fixture_dir = Path::new(FIXTURE_DIR);
    let mut session = spawn_claude(ClaudeRunnerRequest {
        claude_bin: fixture_dir.join("fake_interactive_claude.sh"),
        cwd: fixture_dir.to_path_buf(),
        mode: TaskMode::Research,
        model: None,
        effort: None,
        settings_path: None,
        extra_env: BTreeMap::from([(
            "FAKE_CLAUDE_SCENARIO".to_string(),
            "terminal-probes".to_string(),
        )]),
    })?;

    let mut output = Vec::new();
    let mut buffer = [0_u8; 4096];
    let status = timeout(Duration::from_secs(2), async {
        loop {
            tokio::select! {
                status = session.child.wait() => return status,
                read = session.reader.read(&mut buffer) => match read {
                    Ok(0) => return session.child.wait().await,
                    Ok(count) => output.extend_from_slice(&buffer[..count]),
                    Err(error) if error.raw_os_error() == Some(libc::EIO) => {
                        return session.child.wait().await;
                    }
                    Err(error) => return Err(error),
                }
            }
        }
    })
    .await??;

    assert!(status.success(), "fake Claude exited with {status}");
    assert!(
        output.windows(3).any(|window| window == b"\x1b[c"),
        "missing probe output from PTY-spawned fake Claude: {output:?}"
    );

    Ok(())
}
