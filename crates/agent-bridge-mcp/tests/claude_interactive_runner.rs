use agent_bridge_mcp::claude_interactive::runner::{
    ClaudeRunnerRequest, build_pty_spawn, inject_prompt, spawn_claude,
};
use agent_bridge_mcp::domain::TaskMode;
use std::collections::BTreeMap;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
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
        debug_file: None,
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

#[tokio::test]
async fn claude_runner_injects_prompt_through_pty_not_argv() -> io::Result<()> {
    let fixture_dir = Path::new(FIXTURE_DIR);
    let temp = spike_temp_dir("runner-prompt")?;
    let prompt_log = temp.join("prompt.txt");
    let wrapper = temp.join("fake_prompt_entry.sh");
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nFAKE_CLAUDE_SCENARIO=prompt-entry AGENT_BRIDGE_FAKE_CLAUDE_PROMPT_LOG='{}' exec '{}' \"$@\"\n",
            prompt_log.display(),
            fixture_dir.join("fake_interactive_claude.sh").display()
        ),
    )?;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700))?;
    let prompt = "secret prompt that must not be argv";
    let request = ClaudeRunnerRequest {
        claude_bin: wrapper,
        cwd: fixture_dir.to_path_buf(),
        mode: TaskMode::Research,
        model: None,
        effort: None,
        settings_path: None,
        debug_file: None,
        extra_env: BTreeMap::new(),
    };
    let spawn = build_pty_spawn(ClaudeRunnerRequest {
        claude_bin: request.claude_bin.clone(),
        cwd: request.cwd.clone(),
        mode: request.mode,
        model: request.model.clone(),
        effort: request.effort.clone(),
        settings_path: request.settings_path.clone(),
        debug_file: request.debug_file.clone(),
        extra_env: request.extra_env.clone(),
    });
    assert!(
        spawn.args.iter().all(|arg| !arg.contains(prompt)),
        "prompt leaked into argv: {:?}",
        spawn.args
    );

    let mut session = spawn_claude(request)?;
    let mut ready_output = Vec::new();
    let mut buffer = [0_u8; 256];
    let ready = timeout(Duration::from_secs(2), async {
        loop {
            let count = match session.reader.read(&mut buffer).await {
                Ok(count) => count,
                Err(error) if error.kind() == io::ErrorKind::TimedOut => {
                    tokio::time::sleep(Duration::from_millis(25)).await;
                    continue;
                }
                Err(error) => return Err(error),
            };
            ready_output.extend_from_slice(&buffer[..count]);
            if ready_output
                .windows("prompt-entry-ready".len())
                .any(|window| window == b"prompt-entry-ready")
            {
                return Ok::<(), io::Error>(());
            }
        }
    })
    .await;
    match ready {
        Ok(result) => result?,
        Err(error) => {
            panic!(
                "timed out waiting for fake Claude readiness after reading: {:?}: {error}",
                String::from_utf8_lossy(&ready_output)
            );
        }
    }
    inject_prompt(&mut session.writer, prompt).await?;
    let mut post_prompt_output = Vec::new();
    let status = timeout(Duration::from_secs(2), async {
        loop {
            if let Some(status) = session.child.try_wait()? {
                return Ok::<_, io::Error>(status);
            }
            match session.reader.read(&mut buffer).await {
                Ok(0) => {}
                Ok(count) => post_prompt_output.extend_from_slice(&buffer[..count]),
                Err(error) if error.kind() == io::ErrorKind::TimedOut => {}
                Err(error) if error.raw_os_error() == Some(libc::EIO) => {}
                Err(error) => return Err(error),
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .map_err(io::Error::other)??;
    assert!(status.success(), "fake Claude exited with {status}");
    assert_eq!(tokio::fs::read_to_string(prompt_log).await?, prompt);

    Ok(())
}

fn spike_temp_dir(label: &str) -> io::Result<std::path::PathBuf> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "agent-bridge-{label}-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path)?;
    Ok(path)
}
