//! Child-process supervision helpers: the active process-group registry plus
//! the low-level primitives for grouping, terminating, and naming signals.
//!
//! The registry is modeled as an `ActivePids` value rather than a bare global.
//! Production uses a single process-global instance (`ACTIVE_PIDS`) because the
//! panic hook has no handle to thread through; tests construct isolated local
//! instances so signal-delivery can be exercised without a shared global that
//! would cross-kill other tests' children.

use std::collections::BTreeSet;
use std::sync::Mutex;
use tokio::process::Command as ProcessCommand;

/// Tracks live child process-group ids and can signal them as a group.
pub(crate) struct ActivePids {
    inner: Mutex<BTreeSet<u32>>,
}

impl ActivePids {
    pub(crate) const fn new() -> Self {
        Self {
            inner: Mutex::new(BTreeSet::new()),
        }
    }

    pub(crate) fn register(&self, pid: u32) {
        if let Ok(mut pids) = self.inner.lock() {
            pids.insert(pid);
        }
    }

    pub(crate) fn unregister(&self, pid: u32) {
        if let Ok(mut pids) = self.inner.lock() {
            pids.remove(&pid);
        }
    }

    #[cfg(test)]
    pub(crate) fn contains(&self, pid: u32) -> bool {
        self.inner
            .lock()
            .map(|pids| pids.contains(&pid))
            .unwrap_or(false)
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.inner
            .lock()
            .map(|pids| pids.is_empty())
            .unwrap_or(true)
    }

    /// Best-effort termination of every tracked child process group. Safe to
    /// call from the panic hook: it only takes a lock and issues signals.
    pub(crate) fn terminate_all(&self, signal: i32) {
        let pids: Vec<u32> = self
            .inner
            .lock()
            .map(|pids| pids.iter().copied().collect())
            .unwrap_or_default();
        for pid in pids {
            terminate_child_tree(pid, signal);
        }
    }
}

/// Process-global registry consulted by the panic hook and host shutdown path.
pub(crate) static ACTIVE_PIDS: ActivePids = ActivePids::new();

pub(crate) fn register_active_pid(pid: u32) {
    ACTIVE_PIDS.register(pid);
}

pub(crate) fn unregister_active_pid(pid: u32) {
    ACTIVE_PIDS.unregister(pid);
}

pub(crate) fn terminate_all_active_pids(signal: i32) {
    ACTIVE_PIDS.terminate_all(signal);
}

#[cfg(unix)]
pub(crate) fn configure_child_process_group(command: &mut ProcessCommand) {
    unsafe {
        command.pre_exec(|| {
            if libc::setpgid(0, 0) == 0 {
                Ok(())
            } else {
                Err(std::io::Error::last_os_error())
            }
        });
    }
}

#[cfg(not(unix))]
pub(crate) fn configure_child_process_group(_command: &mut ProcessCommand) {}

#[cfg(unix)]
pub(crate) fn terminate_child_tree(pid: u32, signal: i32) {
    unsafe {
        libc::killpg(pid as libc::pid_t, signal);
    }
}

#[cfg(not(unix))]
pub(crate) fn terminate_child_tree(_pid: u32, _signal: i32) {}

#[cfg(unix)]
pub(crate) fn signal_name(status: &std::process::ExitStatus) -> Option<String> {
    use std::os::unix::process::ExitStatusExt;
    status.signal().map(|signal| match signal {
        libc::SIGTERM => "SIGTERM".to_string(),
        libc::SIGKILL => "SIGKILL".to_string(),
        other => format!("SIG{other}"),
    })
}

#[cfg(not(unix))]
pub(crate) fn signal_name(_status: &std::process::ExitStatus) -> Option<String> {
    None
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::process::Stdio;
    use tokio::time::{Duration, timeout};

    #[test]
    fn registry_tracks_and_clears() {
        let registry = ActivePids::new();
        registry.register(4242);
        assert!(registry.contains(4242));
        registry.unregister(4242);
        assert!(registry.is_empty());
    }

    fn spawn_sleep() -> tokio::process::Child {
        let mut command = ProcessCommand::new("/bin/sleep");
        command
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        configure_child_process_group(&mut command);
        command.spawn().unwrap()
    }

    #[tokio::test]
    async fn terminate_child_tree_sends_sigterm_to_unix_process_group() {
        let mut child = spawn_sleep();
        let pid = child.id().unwrap();
        terminate_child_tree(pid, libc::SIGTERM);
        let status = timeout(Duration::from_secs(3), child.wait())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(signal_name(&status).as_deref(), Some("SIGTERM"));
    }

    #[tokio::test]
    async fn terminate_all_signals_only_registered_children() {
        // A local registry instance is fully isolated from any concurrently
        // running test, so terminating "all" of its children is safe.
        let registry = ActivePids::new();
        let mut child = spawn_sleep();
        let pid = child.id().unwrap();
        registry.register(pid);

        registry.terminate_all(libc::SIGTERM);
        let status = timeout(Duration::from_secs(3), child.wait())
            .await
            .unwrap()
            .unwrap();
        registry.unregister(pid);

        assert_eq!(signal_name(&status).as_deref(), Some("SIGTERM"));
        assert!(registry.is_empty());
    }
}

use super::complete::{append_transcript_event, diagnostic_redactions};
use super::review::parse_transcript_line;
use super::*;

pub(super) struct ChildIoDrains {
    pub stdout: Option<tokio::task::JoinHandle<()>>,
    pub stderr: Option<tokio::task::JoinHandle<()>>,
}

/// Appends one transcript event per non-blank line of a host-runner stream
/// (stdout/stderr), classifying each line via `parse_transcript_line`.
pub(super) async fn append_stream_transcript(
    transcript_path: &Path,
    provider: ProviderKind,
    stream: &'static str,
    text: &str,
    redactions: &[String],
) {
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let (kind, parsed) = parse_transcript_line(line);
        append_transcript_event(
            transcript_path,
            provider,
            stream,
            kind,
            line,
            parsed,
            redactions,
        )
        .await;
    }
}

pub(super) async fn wait_for_child(
    agent_id: String,
    pid: u32,
    timeout_seconds: i64,
    mut child: tokio::process::Child,
    command: ProviderCommand,
    agent_dir: PathBuf,
    drains: ChildIoDrains,
) -> TaskCompletion {
    let wait = child.wait();
    tokio::pin!(wait);
    let agent_timeout = sleep(Duration::from_secs(timeout_seconds as u64));
    tokio::pin!(agent_timeout);
    let mut timed_out = false;
    let mut fatal_denial = false;
    let stderr_path = agent_dir.join("stderr.log");
    let output: Result<std::process::ExitStatus, String> = loop {
        tokio::select! {
            wait_result = &mut wait => {
                break wait_result.map_err(|error| error.to_string());
            }
            _ = &mut agent_timeout => {
                timed_out = true;
                terminate_child_tree(pid, libc::SIGTERM);
                break match timeout(CHILD_SHUTDOWN_GRACE, &mut wait).await {
                    Ok(result) => result.map_err(|error| error.to_string()),
                    Err(_) => {
                        terminate_child_tree(pid, libc::SIGKILL);
                        match timeout(SIGKILL_REAP_GRACE, &mut wait).await {
                            Ok(result) => result.map_err(|error| error.to_string()),
                            Err(_) => Err(format!(
                                "child process group {pid} did not exit within {}s after SIGKILL; reporting best-effort status",
                                SIGKILL_REAP_GRACE.as_secs()
                            )),
                        }
                    }
                };
            }
            _ = sleep(Duration::from_millis(50)), if provider::adapter_for(command.provider).polls_stderr_for_denial() => {
                let stderr = fs::read(&stderr_path).await.unwrap_or_default();
                if provider::adapter_for(command.provider).detects_fatal_denial(&stderr) {
                    fatal_denial = true;
                    terminate_child_tree(pid, libc::SIGTERM);
                    break match timeout(CHILD_SHUTDOWN_GRACE, &mut wait).await {
                        Ok(result) => result,
                        Err(_) => {
                            terminate_child_tree(pid, libc::SIGKILL);
                            (&mut wait).await
                        }
                    }
                    .map_err(|error| error.to_string());
                }
            }
        }
    };
    if let Some(handle) = drains.stdout {
        let _ = timeout(CHILD_SHUTDOWN_GRACE, handle).await;
    }
    if let Some(handle) = drains.stderr {
        let _ = timeout(CHILD_SHUTDOWN_GRACE, handle).await;
    }
    let (exit_code, signal, wait_error) = match &output {
        Ok(status) => (status.code(), signal_name(status), None),
        Err(error) => (None, None, Some(error.clone())),
    };
    if timed_out {
        append_transcript_event(
            &agent_dir.join("transcript.jsonl"),
            command.provider,
            "lifecycle",
            "lifecycle",
            "",
            json!({"phase": "timeout", "timeoutSeconds": timeout_seconds, "profile": command.profile}),
            &diagnostic_redactions(&command),
        )
        .await;
    }
    append_transcript_event(
        &agent_dir.join("transcript.jsonl"),
        command.provider,
        "lifecycle",
        "lifecycle",
        "",
        json!({
            "phase": "exited",
            "exitCode": exit_code,
            "signal": signal,
            "error": wait_error,
            "timedOut": timed_out,
            "profile": command.profile
        }),
        &diagnostic_redactions(&command),
    )
    .await;
    classify_completion(
        agent_id,
        &command,
        &agent_dir,
        timeout_seconds,
        output,
        timed_out,
        fatal_denial,
    )
}

pub(super) async fn drain_log(
    mut reader: impl tokio::io::AsyncRead + Unpin,
    path: PathBuf,
    transcript_path: PathBuf,
    provider: ProviderKind,
    source: &'static str,
    redactions: Vec<String>,
) {
    let mut file_bytes = 0usize;
    let mut saw_output = false;
    let mut buffer = [0u8; 8192];
    while let Ok(count) = reader.read(&mut buffer).await {
        if count == 0 {
            break;
        }
        if file_bytes >= MAX_LOG_BYTES {
            continue;
        }
        let remaining = MAX_LOG_BYTES - file_bytes;
        let take = remaining.min(count);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent).await;
        }
        if let Ok(mut file) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
        {
            use tokio::io::AsyncWriteExt;
            let _ = file.write_all(&buffer[..take]).await;
        }
        let text = String::from_utf8_lossy(&buffer[..take]).to_string();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            if !saw_output {
                append_transcript_event(
                    &transcript_path,
                    provider,
                    "lifecycle",
                    "lifecycle",
                    "",
                    json!({"phase": "first_output", "source": source}),
                    &redactions,
                )
                .await;
                saw_output = true;
            }
            let (kind, parsed) = parse_transcript_line(line);
            append_transcript_event(
                &transcript_path,
                provider,
                source,
                kind,
                line,
                parsed,
                &redactions,
            )
            .await;
        }
        file_bytes += take;
    }
    if saw_output {
        append_transcript_event(
            &transcript_path,
            provider,
            "lifecycle",
            "lifecycle",
            "",
            json!({"phase": "final_output", "source": source}),
            &redactions,
        )
        .await;
    }
}
