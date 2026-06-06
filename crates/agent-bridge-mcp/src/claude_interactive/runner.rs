use crate::claude_interactive::failure::parse_stop_failure;
use crate::claude_interactive::hooks::{
    HookRelay, HookRelayEvent, parse_event_line, write_temporary_settings,
};
use crate::claude_interactive::pty::{PtySession, PtySize, PtySpawn, spawn};
use crate::claude_interactive::setup::detect_setup_prompt;
use crate::claude_interactive::terminal::TerminalProbeHandler;
use crate::claude_interactive::transcript::{TranscriptSource, resolve_stop_result};
use crate::domain::TaskMode;
use crate::provider;
use std::collections::BTreeMap;
use std::fs::File;
use std::io;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use uuid::Uuid;

const LOGIN_SHELL: &str = "/bin/zsh";
const LOGIN_SHELL_BOOTSTRAP: &str = "exec \"$@\"";
const LOGIN_SHELL_ARG0: &str = "agent-bridge-claude";
const PROMPT_ENTER_DELAY: Duration = Duration::from_millis(100);
const TRANSCRIPT_RETRY_BUDGET: Duration = Duration::from_secs(2);
const PTY_EXCERPT_LIMIT: usize = 64 * 1024;
const STOP_EVENT_GRACE: Duration = Duration::from_secs(12);

pub struct ClaudeRunnerRequest {
    pub claude_bin: PathBuf,
    pub cwd: PathBuf,
    pub mode: TaskMode,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub settings_path: Option<PathBuf>,
    pub debug_file: Option<PathBuf>,
    pub extra_env: BTreeMap<String, String>,
}

pub struct ClaudeInteractiveRunRequest {
    pub claude_bin: PathBuf,
    pub cwd: PathBuf,
    pub timeout_seconds: i64,
    pub mode: TaskMode,
    pub prompt: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub extra_env: BTreeMap<String, String>,
    pub disconnect: Option<oneshot::Receiver<()>>,
}

#[derive(Debug)]
pub struct ClaudeInteractiveRunResult {
    pub exit_code: Option<i32>,
    pub signal: Option<String>,
    pub final_text: Option<String>,
    pub final_text_source: Option<String>,
    pub session_id: Option<String>,
    pub failure_category: Option<String>,
    pub pty_output_excerpt: String,
    pub pty_output_truncated: bool,
    pub stop: Option<serde_json::Value>,
    pub stop_failure: Option<serde_json::Value>,
    pub transcript: serde_json::Value,
    pub duration_ms: u64,
}

pub fn spawn_claude(request: ClaudeRunnerRequest) -> io::Result<PtySession> {
    spawn(build_pty_spawn(request))
}

pub async fn run_interactive(
    request: ClaudeInteractiveRunRequest,
) -> io::Result<ClaudeInteractiveRunResult> {
    let started = std::time::Instant::now();
    let run_dir = std::env::temp_dir().join(format!("agent-bridge-claude-run-{}", Uuid::new_v4()));
    let debug_enabled = std::env::var("AGENT_BRIDGE_CLAUDE_RUNNER_DEBUG")
        .ok()
        .as_deref()
        == Some("1");
    let debug_file = debug_enabled.then(|| run_dir.join("claude-debug.log"));
    let relay = HookRelay::prepare(&run_dir)?;
    let settings = write_temporary_settings(&run_dir, &relay.helper_path)?;
    let (event_tx, mut event_rx) = mpsc::channel(16);
    let relay_stop = Arc::new(AtomicBool::new(false));
    let relay_stop_task = Arc::clone(&relay_stop);
    let relay_event_log = relay.event_log_path.clone();
    let relay_task = tokio::task::spawn_blocking(move || {
        read_relay_events(relay_event_log, event_tx, relay_stop_task)
    });
    let mut extra_env = request.extra_env;
    extra_env.extend(relay.env());
    // Deterministic tests use the fake Claude fixture, which can write hook
    // events directly to this sink. The real Claude CLI ignores this variable.
    extra_env.insert(
        "AGENT_BRIDGE_FAKE_CLAUDE_HOOK_SINK".to_string(),
        relay.event_log_path.display().to_string(),
    );
    let mut session = spawn_claude(ClaudeRunnerRequest {
        claude_bin: request.claude_bin,
        cwd: request.cwd,
        mode: request.mode,
        model: request.model,
        effort: request.effort,
        settings_path: Some(settings.settings_path.clone()),
        debug_file: debug_file.clone(),
        extra_env,
    })?;

    let mut terminal = TerminalProbeHandler::new();
    let mut pty_excerpt = Vec::new();
    let mut pty_truncated = false;
    let mut buffer = [0_u8; 4096];
    let mut stop = None;
    let mut stop_failure = None;
    let mut session_start = None;
    let mut exit_status: Option<ExitStatus>;
    let mut prompt_injected = false;
    let mut disconnect = request.disconnect;
    let timeout_sleep = tokio::time::sleep(Duration::from_secs(request.timeout_seconds as u64));
    tokio::pin!(timeout_sleep);

    let failure_category = loop {
        tokio::select! {
            _ = &mut timeout_sleep => {
                exit_status = Some(session.terminate_with_grace(Duration::from_secs(3)).await?);
                break Some("runner_timeout".to_string());
            }
            _ = wait_for_disconnect(&mut disconnect) => {
                exit_status = Some(session.terminate_with_grace(Duration::from_secs(3)).await?);
                break Some("client_disconnected".to_string());
            }
            event = event_rx.recv() => {
                match event {
                    Some(event) if event.event_name == "SessionStart" => {
                        if session_start.is_none() {
                            session_start = Some(event.payload);
                        }
                        if !prompt_injected {
                            inject_prompt(&mut session.writer, &request.prompt).await?;
                            prompt_injected = true;
                        }
                    }
                    Some(event) if event.event_name == "Stop" => {
                        stop = Some(event.payload);
                        exit_status =
                            Some(session.terminate_with_grace(Duration::from_secs(3)).await?);
                        break None;
                    }
                    Some(event) if event.event_name == "StopFailure" => {
                        stop_failure = Some(event.payload);
                        exit_status =
                            Some(session.terminate_with_grace(Duration::from_secs(3)).await?);
                        break Some("claude_api_error".to_string());
                    }
                    Some(_) => {}
                    None => {}
                }
            }
            read = session.reader.read(&mut buffer) => {
                match read {
                    Ok(0) => {
                        if let Some((status, category)) = finish_if_child_exited(
                            &mut session,
                            &mut event_rx,
                            &mut stop,
                            &mut stop_failure,
                        ).await? {
                            exit_status = Some(status);
                            break category;
                        }
                        tokio::time::sleep(Duration::from_millis(25)).await;
                    }
                    Ok(count) => {
                        let chunk = terminal.process(&buffer[..count]);
                        for response in chunk.responses {
                            session.writer.write_all(&response).await?;
                            session.writer.flush().await?;
                        }
                        if let Some(_signature) = detect_setup_prompt(&chunk.output) {
                            append_excerpt(&mut pty_excerpt, &mut pty_truncated, &chunk.output, &request.prompt);
                            exit_status = Some(session.terminate_with_grace(Duration::from_secs(3)).await?);
                            break Some("claude_setup_required".to_string());
                        }
                        append_excerpt(&mut pty_excerpt, &mut pty_truncated, &chunk.output, &request.prompt);
                    }
                    Err(error) if error.raw_os_error() == Some(libc::EIO) => {
                        if let Some((status, category)) = finish_if_child_exited(
                            &mut session,
                            &mut event_rx,
                            &mut stop,
                            &mut stop_failure,
                        ).await? {
                            exit_status = Some(status);
                            break category;
                        }
                        tokio::time::sleep(Duration::from_millis(25)).await;
                    }
                    Err(error) if error.kind() == io::ErrorKind::TimedOut => {
                        tokio::time::sleep(Duration::from_millis(25)).await;
                    }
                    Err(error) => return Err(error),
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(25)) => {
                if let Some((status, category)) = finish_if_child_exited(
                    &mut session,
                    &mut event_rx,
                    &mut stop,
                    &mut stop_failure,
                ).await? {
                    exit_status = Some(status);
                    break category;
                }
            }
        }
    };
    if exit_status.is_none() {
        exit_status = tokio::time::timeout(Duration::from_secs(3), session.child.wait())
            .await
            .ok()
            .and_then(Result::ok);
    }
    relay_stop.store(true, Ordering::Relaxed);
    if debug_enabled {
        eprintln!(
            "agent-bridge claude runner debug artifacts kept at {}",
            relay.run_dir.display()
        );
    } else {
        let _ = relay.cleanup();
    }
    let _ = tokio::time::timeout(Duration::from_secs(1), relay_task).await;

    let mut final_text = None;
    let mut final_text_source = None;
    let mut session_id = None;
    let mut transcript = serde_json::json!({
        "parseStatus": "not_started",
        "fallbackUsed": false
    });
    let failure_category = if let Some(stop_failure_payload) = stop_failure.as_ref() {
        if let Some(failure) = parse_stop_failure(stop_failure_payload) {
            transcript = serde_json::json!({
                "parseStatus": "stop_failure",
                "fallbackUsed": false
            });
            Some(failure.category.as_str().to_string())
        } else {
            failure_category
        }
    } else if let Some(completion_payload) =
        completion_payload(stop.as_ref(), session_start.as_ref())
    {
        match resolve_stop_result(completion_payload, TRANSCRIPT_RETRY_BUDGET).await {
            Ok(result) => {
                final_text = Some(result.final_text);
                final_text_source = Some(match result.source {
                    TranscriptSource::Transcript => "transcript".to_string(),
                    TranscriptSource::StopLastAssistantMessage => {
                        "stop_last_assistant_message".to_string()
                    }
                });
                session_id = result.session_id;
                transcript = serde_json::json!({
                    "parseStatus": "ok",
                    "fallbackUsed": result.fallback_used,
                    "transcriptPathAccepted": result.transcript_path.is_some()
                });
                None
            }
            Err(error) => {
                transcript = serde_json::json!({
                    "parseStatus": "error",
                    "fallbackUsed": false,
                    "diagnostic": error.to_string()
                });
                Some("provider_output_error".to_string())
            }
        }
    } else {
        failure_category
    };

    Ok(ClaudeInteractiveRunResult {
        exit_code: exit_status.as_ref().and_then(ExitStatus::code),
        signal: signal_name(exit_status.as_ref()),
        final_text,
        final_text_source,
        session_id,
        failure_category,
        pty_output_excerpt: String::from_utf8_lossy(&pty_excerpt).to_string(),
        pty_output_truncated: pty_truncated,
        stop,
        stop_failure,
        transcript,
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

pub async fn inject_prompt(writer: &mut (impl AsyncWrite + Unpin), prompt: &str) -> io::Result<()> {
    write_all_retrying_timed_out(writer, prompt.as_bytes()).await?;
    flush_retrying_timed_out(writer).await?;
    tokio::time::sleep(PROMPT_ENTER_DELAY).await;
    write_all_retrying_timed_out(writer, b"\r\n").await?;
    flush_retrying_timed_out(writer).await
}

async fn write_all_retrying_timed_out(
    writer: &mut (impl AsyncWrite + Unpin),
    bytes: &[u8],
) -> io::Result<()> {
    let mut written = 0;
    while written < bytes.len() {
        match writer.write(&bytes[written..]).await {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(count) => written += count,
            Err(error) if error.kind() == io::ErrorKind::TimedOut => {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

async fn flush_retrying_timed_out(writer: &mut (impl AsyncWrite + Unpin)) -> io::Result<()> {
    loop {
        match writer.flush().await {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::TimedOut => {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Err(error) => return Err(error),
        }
    }
}

fn completion_payload<'a>(
    stop: Option<&'a serde_json::Value>,
    session_start: Option<&'a serde_json::Value>,
) -> Option<&'a serde_json::Value> {
    stop.filter(|payload| payload.get("transcript_path").is_some())
        .or(session_start)
        .or(stop)
}

async fn wait_for_disconnect(disconnect: &mut Option<oneshot::Receiver<()>>) {
    match disconnect {
        Some(receiver) => {
            let _ = receiver.await;
        }
        None => std::future::pending::<()>().await,
    }
}

async fn finish_if_child_exited(
    session: &mut PtySession,
    event_rx: &mut mpsc::Receiver<HookRelayEvent>,
    stop: &mut Option<serde_json::Value>,
    stop_failure: &mut Option<serde_json::Value>,
) -> io::Result<Option<(ExitStatus, Option<String>)>> {
    let Some(status) = session.child.try_wait()? else {
        return Ok(None);
    };
    if stop.is_none() && stop_failure.is_none() {
        // Claude can finish rendering the final response before its Stop hooks
        // complete. Treat child exit as the start of a bounded hook-drain window,
        // not immediate proof that no transcript result will arrive.
        match wait_for_completion_event(event_rx, STOP_EVENT_GRACE).await {
            Some(event) if event.event_name == "Stop" => {
                *stop = Some(event.payload);
                return Ok(Some((status, None)));
            }
            Some(event) if event.event_name == "StopFailure" => {
                *stop_failure = Some(event.payload);
                return Ok(Some((status, Some("claude_api_error".to_string()))));
            }
            _ => {
                return Ok(Some((status, Some("provider_output_error".to_string()))));
            }
        }
    }
    let category = if stop_failure.is_some() {
        Some("claude_api_error".to_string())
    } else {
        None
    };
    Ok(Some((status, category)))
}

async fn wait_for_completion_event(
    event_rx: &mut mpsc::Receiver<HookRelayEvent>,
    duration: Duration,
) -> Option<HookRelayEvent> {
    let deadline = tokio::time::sleep(duration);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => return None,
            event = event_rx.recv() => match event {
                Some(event) if matches!(event.event_name.as_str(), "Stop" | "StopFailure") => {
                    return Some(event);
                }
                Some(_) => {}
                None => return None,
            }
        }
    }
}

fn read_relay_events(path: PathBuf, tx: mpsc::Sender<HookRelayEvent>, stop: Arc<AtomicBool>) {
    let mut line = Vec::new();
    let mut offset = 0;
    while !stop.load(Ordering::Relaxed) {
        let mut reader = match File::open(&path) {
            Ok(reader) => reader,
            Err(_) => {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
        };
        if reader.seek(SeekFrom::Start(offset)).is_err() {
            return;
        }
        let mut bytes = Vec::new();
        match reader.read_to_end(&mut bytes) {
            Ok(0) => std::thread::sleep(Duration::from_millis(10)),
            Ok(count) => {
                offset += count as u64;
                for byte in bytes {
                    line.push(byte);
                    if byte == b'\n' {
                        if let Ok(event) = parse_event_line(&line)
                            && tx.blocking_send(event).is_err()
                        {
                            return;
                        }
                        line.clear();
                    }
                }
            }
            Err(_) => return,
        }
    }
}

fn append_excerpt(excerpt: &mut Vec<u8>, truncated: &mut bool, bytes: &[u8], prompt: &str) {
    let mut redacted = bytes.to_vec();
    if !prompt.is_empty() {
        redacted = String::from_utf8_lossy(&redacted)
            .replace(prompt, "[REDACTED_PROMPT]")
            .into_bytes();
    }
    let remaining = PTY_EXCERPT_LIMIT.saturating_sub(excerpt.len());
    if remaining == 0 {
        *truncated = true;
        return;
    }
    let take = remaining.min(redacted.len());
    excerpt.extend_from_slice(&redacted[..take]);
    if take < redacted.len() {
        *truncated = true;
    }
}

#[cfg(unix)]
fn signal_name(status: Option<&ExitStatus>) -> Option<String> {
    use std::os::unix::process::ExitStatusExt;
    status.and_then(|status| {
        status.signal().map(|signal| match signal {
            libc::SIGTERM => "SIGTERM".to_string(),
            libc::SIGKILL => "SIGKILL".to_string(),
            other => format!("SIG{other}"),
        })
    })
}

#[cfg(not(unix))]
fn signal_name(_status: Option<&ExitStatus>) -> Option<String> {
    None
}

pub fn build_pty_spawn(request: ClaudeRunnerRequest) -> PtySpawn {
    let mut args = vec![
        "-flc".to_string(),
        LOGIN_SHELL_BOOTSTRAP.to_string(),
        LOGIN_SHELL_ARG0.to_string(),
        request.claude_bin.display().to_string(),
        "--setting-sources".to_string(),
        "local".to_string(),
    ];
    args.extend(mode_flags(request.mode));
    if let Some(settings_path) = request.settings_path {
        args.extend([
            "--settings".to_string(),
            settings_path.display().to_string(),
        ]);
    }
    if let Some(debug_file) = request.debug_file {
        args.extend([
            "--debug".to_string(),
            "hooks".to_string(),
            "--debug-file".to_string(),
            debug_file.display().to_string(),
        ]);
    }
    if let Some(model) = request.model {
        args.extend(["--model".to_string(), model]);
    }
    if let Some(effort) = request.effort {
        args.extend(["--effort".to_string(), effort]);
    }
    let mut env = provider::provider_env(crate::domain::ProviderKind::Claude);
    env.extend(request.extra_env);
    PtySpawn {
        program: Path::new(LOGIN_SHELL).to_path_buf(),
        args,
        cwd: request.cwd,
        env,
        size: PtySize {
            rows: 40,
            cols: 120,
        },
        resize_after_open: None,
    }
}

fn mode_flags(mode: TaskMode) -> Vec<String> {
    match mode {
        TaskMode::Research | TaskMode::Review => vec![
            "--permission-mode".to_string(),
            "dontAsk".to_string(),
            "--tools".to_string(),
            "Read,Grep,Glob".to_string(),
            "--allowedTools".to_string(),
            "Read,Grep,Glob".to_string(),
            "--disallowedTools".to_string(),
            "Bash,Edit,Write".to_string(),
        ],
        TaskMode::Command => vec![
            "--permission-mode".to_string(),
            "default".to_string(),
            "--tools".to_string(),
            "Read,Grep,Glob,Bash".to_string(),
            "--allowedTools".to_string(),
            "Read,Grep,Glob,Bash".to_string(),
            "--disallowedTools".to_string(),
            "Edit,Write".to_string(),
        ],
        TaskMode::Implement => vec!["--permission-mode".to_string(), "default".to_string()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_spawn_uses_fixed_login_shell_and_interactive_flags() {
        let spawn = build_pty_spawn(ClaudeRunnerRequest {
            claude_bin: PathBuf::from("/usr/local/bin/claude"),
            cwd: PathBuf::from("/tmp/workspace"),
            mode: TaskMode::Research,
            model: Some("sonnet".to_string()),
            effort: Some("high".to_string()),
            settings_path: Some(PathBuf::from("/tmp/settings.json")),
            debug_file: Some(PathBuf::from("/tmp/claude-debug.log")),
            extra_env: BTreeMap::new(),
        });

        assert_eq!(spawn.program, PathBuf::from(LOGIN_SHELL));
        assert_eq!(spawn.args[0], "-flc");
        assert_eq!(spawn.args[1], LOGIN_SHELL_BOOTSTRAP);
        assert_eq!(spawn.args[2], LOGIN_SHELL_ARG0);
        assert_eq!(spawn.args[3], "/usr/local/bin/claude");
        assert!(spawn.args.contains(&"--setting-sources".to_string()));
        assert!(spawn.args.contains(&"local".to_string()));
        assert!(spawn.args.contains(&"--permission-mode".to_string()));
        assert!(spawn.args.contains(&"dontAsk".to_string()));
        assert!(spawn.args.contains(&"--tools".to_string()));
        assert!(spawn.args.contains(&"Read,Grep,Glob".to_string()));
        assert!(spawn.args.contains(&"--settings".to_string()));
        assert!(spawn.args.contains(&"/tmp/settings.json".to_string()));
        assert!(spawn.args.contains(&"--debug".to_string()));
        assert!(spawn.args.contains(&"hooks".to_string()));
        assert!(spawn.args.contains(&"--debug-file".to_string()));
        assert!(spawn.args.contains(&"/tmp/claude-debug.log".to_string()));
        assert!(spawn.args.contains(&"--model".to_string()));
        assert!(spawn.args.contains(&"sonnet".to_string()));
        assert!(spawn.args.contains(&"--effort".to_string()));
        assert!(spawn.args.contains(&"high".to_string()));
    }

    #[tokio::test]
    async fn owned_runner_completes_fake_claude_success() {
        let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("interactive_claude");
        let result = run_interactive(ClaudeInteractiveRunRequest {
            claude_bin: fixture_dir.join("fake_interactive_claude.sh"),
            cwd: fixture_dir,
            timeout_seconds: 5,
            mode: TaskMode::Research,
            prompt: "runner prompt".to_string(),
            model: None,
            effort: None,
            extra_env: BTreeMap::from([(
                "FAKE_CLAUDE_SCENARIO".to_string(),
                "success".to_string(),
            )]),
            disconnect: None,
        })
        .await
        .unwrap();

        assert_eq!(result.failure_category, None);
        assert_eq!(result.final_text.as_deref(), Some("fixture final response"));
        assert_eq!(result.final_text_source.as_deref(), Some("transcript"));
        assert_eq!(result.transcript["parseStatus"], "ok");
        assert!(!result.pty_output_excerpt.contains("runner prompt"));
    }

    #[tokio::test]
    async fn owned_runner_terminates_repl_after_stop_event() {
        let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("interactive_claude");
        let result = run_interactive(ClaudeInteractiveRunRequest {
            claude_bin: fixture_dir.join("fake_interactive_claude.sh"),
            cwd: fixture_dir,
            timeout_seconds: 10,
            mode: TaskMode::Research,
            prompt: "runner prompt".to_string(),
            model: None,
            effort: None,
            extra_env: BTreeMap::from([(
                "FAKE_CLAUDE_SCENARIO".to_string(),
                "stop-stays-open".to_string(),
            )]),
            disconnect: None,
        })
        .await
        .unwrap();

        assert_eq!(result.failure_category, None);
        assert_eq!(result.final_text.as_deref(), Some("fixture final response"));
        assert_eq!(result.transcript["parseStatus"], "ok");
    }

    #[tokio::test]
    async fn owned_runner_detects_setup_prompt() {
        let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("interactive_claude");
        let result = run_interactive(ClaudeInteractiveRunRequest {
            claude_bin: fixture_dir.join("fake_interactive_claude.sh"),
            cwd: fixture_dir,
            timeout_seconds: 5,
            mode: TaskMode::Research,
            prompt: "runner prompt".to_string(),
            model: None,
            effort: None,
            extra_env: BTreeMap::from([(
                "FAKE_CLAUDE_SCENARIO".to_string(),
                "setup-login".to_string(),
            )]),
            disconnect: None,
        })
        .await
        .unwrap();

        assert_eq!(
            result.failure_category.as_deref(),
            Some("claude_setup_required")
        );
        assert!(result.final_text.is_none());
        assert!(!result.pty_output_excerpt.contains("runner prompt"));
    }
}
