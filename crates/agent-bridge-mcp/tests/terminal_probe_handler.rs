use agent_bridge_mcp::claude_interactive::pty::{PtySize, PtySpawn, spawn};
use agent_bridge_mcp::claude_interactive::terminal::TerminalProbeHandler;
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
async fn terminal_probe_handler_filters_fake_claude_probe_noise() -> io::Result<()> {
    let fixture_dir = Path::new(FIXTURE_DIR);
    let mut session = spawn(PtySpawn {
        program: fixture_dir.join("fake_interactive_claude.sh"),
        args: Vec::new(),
        cwd: fixture_dir.to_path_buf(),
        env: BTreeMap::from([(
            "FAKE_CLAUDE_SCENARIO".to_string(),
            "terminal-probes".to_string(),
        )]),
        size: PtySize {
            rows: 40,
            cols: 120,
        },
        resize_after_open: None,
    })?;

    let mut handler = TerminalProbeHandler::new();
    let mut filtered = Vec::new();
    let mut responses = Vec::new();
    let mut buffer = [0_u8; 4096];
    timeout(Duration::from_secs(2), async {
        loop {
            tokio::select! {
                status = session.child.wait() => return status.map(|status| {
                    assert!(status.success(), "fake Claude exited with {status}");
                }),
                read = session.reader.read(&mut buffer) => match read {
                    Ok(0) => return Ok(()),
                    Ok(count) => {
                        let chunk = handler.process(&buffer[..count]);
                        filtered.extend(chunk.output);
                        responses.extend(chunk.responses);
                    }
                    Err(error) if error.raw_os_error() == Some(libc::EIO) => return Ok(()),
                    Err(error) => return Err(error),
                }
            }
        }
    })
    .await??;
    filtered.extend(handler.finish());

    assert!(
        filtered.is_empty(),
        "probe bytes leaked into output: {filtered:?}"
    );
    assert_eq!(responses.len(), 5);
    assert!(responses.contains(&b"\x1b[?1;2c".to_vec()));
    assert!(responses.contains(&b"\x1b[>0;0;0c".to_vec()));
    assert!(responses.contains(&b"\x1b[1;1R".to_vec()));
    assert!(responses.contains(&b"\x1bP>|agent-bridge-claude\x1b\\".to_vec()));
    assert!(responses.contains(&b"\x1b[8;40;120t".to_vec()));

    Ok(())
}
