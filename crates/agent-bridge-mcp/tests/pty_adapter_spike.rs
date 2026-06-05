use agent_bridge_mcp::claude_interactive::pty::{
    PtySession, PtySize, PtySpawn, spawn, terminate_process_group,
};
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::timeout;

const FIXTURE_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/interactive_claude"
);

#[tokio::test]
async fn pty_adapter_reads_probe_bytes_and_writes_prompt_through_split_halves() -> io::Result<()> {
    let fixture_dir = Path::new(FIXTURE_DIR);
    let script = fixture_dir.join("fake_interactive_claude.sh");
    let temp = spike_temp_dir("probe-prompt")?;
    let prompt_log = temp.join("prompt.txt");

    let mut session = spawn(PtySpawn {
        program: script,
        args: Vec::new(),
        cwd: fixture_dir.to_path_buf(),
        env: BTreeMap::from([
            (
                "FAKE_CLAUDE_SCENARIO".to_string(),
                "prompt-entry".to_string(),
            ),
            (
                "AGENT_BRIDGE_FAKE_CLAUDE_PROMPT_LOG".to_string(),
                prompt_log.display().to_string(),
            ),
        ]),
        size: PtySize {
            rows: 40,
            cols: 120,
        },
        resize_after_open: Some(PtySize {
            rows: 30,
            cols: 100,
        }),
    })?;

    session.writer.write_all(b"fixture prompt via pty").await?;
    session.writer.write_all(b"\r").await?;
    session.writer.flush().await?;

    let (output, status) = read_until_exit(&mut session, Duration::from_secs(2)).await?;
    assert!(status.success(), "fake Claude exited with {status}");
    assert!(
        output
            .windows("prompt captured".len())
            .any(|window| window == b"prompt captured"),
        "missing prompt completion output: {}",
        String::from_utf8_lossy(&output)
    );
    assert_eq!(
        std::fs::read_to_string(prompt_log)?,
        "fixture prompt via pty"
    );

    let mut probes = spawn(PtySpawn {
        program: fixture_dir.join("fake_interactive_claude.sh"),
        args: Vec::new(),
        cwd: fixture_dir.to_path_buf(),
        env: BTreeMap::from([(
            "FAKE_CLAUDE_SCENARIO".to_string(),
            "terminal-probes".to_string(),
        )]),
        size: PtySize { rows: 24, cols: 80 },
        resize_after_open: None,
    })?;
    let (probe_output, probe_status) = read_until_exit(&mut probes, Duration::from_secs(2)).await?;
    assert!(
        probe_status.success(),
        "probe fixture exited with {probe_status}"
    );
    for expected in [
        b"\x1b[c".as_slice(),
        b"\x1b[>c".as_slice(),
        b"\x1b[6n".as_slice(),
        b"\x1b[>q".as_slice(),
        b"\x1b[18t".as_slice(),
    ] {
        assert!(
            probe_output
                .windows(expected.len())
                .any(|window| window == expected),
            "missing probe bytes {expected:?} in {probe_output:?}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn pty_adapter_can_terminate_child_process_group() -> io::Result<()> {
    let fixture_dir = Path::new(FIXTURE_DIR);
    let temp = spike_temp_dir("cleanup")?;
    let cleanup_marker = temp.join("cleanup.txt");
    let mut session = spawn(PtySpawn {
        program: fixture_dir.join("fake_interactive_claude.sh"),
        args: Vec::new(),
        cwd: fixture_dir.to_path_buf(),
        env: BTreeMap::from([
            (
                "FAKE_CLAUDE_SCENARIO".to_string(),
                "child-cleanup".to_string(),
            ),
            (
                "AGENT_BRIDGE_FAKE_CLAUDE_CLEANUP_MARKER".to_string(),
                cleanup_marker.display().to_string(),
            ),
        ]),
        size: PtySize {
            rows: 40,
            cols: 120,
        },
        resize_after_open: Some(PtySize {
            rows: 40,
            cols: 100,
        }),
    })?;
    let pid = session
        .pid
        .ok_or_else(|| io::Error::other("PTY child did not expose pid"))?;

    tokio::time::sleep(Duration::from_millis(150)).await;
    terminate_process_group(pid, libc::SIGTERM);
    let _ = timeout(Duration::from_secs(3), session.child.wait()).await??;

    let marker = wait_for_file(&cleanup_marker, Duration::from_secs(3)).await?;
    assert!(
        marker.contains("child-terminated") || marker.contains("parent-terminated"),
        "cleanup marker did not show signal handling: {marker:?}"
    );

    Ok(())
}

async fn read_until_exit(
    session: &mut PtySession,
    duration: Duration,
) -> io::Result<(Vec<u8>, std::process::ExitStatus)> {
    timeout(duration, async {
        let mut output = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            tokio::select! {
                status = session.child.wait() => {
                    drain_reader(&mut session.reader, &mut output).await?;
                    return Ok((output, status?));
                }
                read = session.reader.read(&mut buffer) => {
                    match read {
                        Ok(0) => {
                            let status = session.child.wait().await?;
                            return Ok((output, status));
                        }
                        Ok(count) => output.extend_from_slice(&buffer[..count]),
                        Err(error) if is_pty_eof(&error) => {
                            let status = session.child.wait().await?;
                            return Ok((output, status));
                        }
                        Err(error) => return Err(error),
                    }
                }
            }
        }
    })
    .await?
}

async fn drain_reader(
    reader: &mut pty_process::OwnedReadPty,
    output: &mut Vec<u8>,
) -> io::Result<()> {
    let mut buffer = [0_u8; 4096];
    loop {
        match timeout(Duration::from_millis(50), reader.read(&mut buffer)).await {
            Ok(Ok(0)) | Err(_) => return Ok(()),
            Ok(Ok(count)) => output.extend_from_slice(&buffer[..count]),
            Ok(Err(error)) if is_pty_eof(&error) => return Ok(()),
            Ok(Err(error)) => return Err(error),
        }
    }
}

fn is_pty_eof(error: &io::Error) -> bool {
    error.raw_os_error() == Some(libc::EIO)
}

async fn wait_for_file(path: &Path, duration: Duration) -> io::Result<String> {
    timeout(duration, async {
        loop {
            match tokio::fs::read_to_string(path).await {
                Ok(text) => return Ok(text),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
                Err(error) => return Err(error),
            }
        }
    })
    .await?
}

fn spike_temp_dir(label: &str) -> io::Result<PathBuf> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "agent-bridge-pty-spike-{label}-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path)?;
    Ok(path)
}
