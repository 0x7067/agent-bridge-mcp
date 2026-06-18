use super::complete::{
    agent_diagnostic, append_transcript_event, codex_denial_completion, command_provider_hint,
};
use super::supervision::{
    DrainLogContext, StderrDenialScanner, configure_child_process_group, drain_log,
    register_active_pid, terminate_child_tree, unregister_active_pid,
};
use super::{ActiveTask, ActorCommand, TaskCompletion};
use crate::domain::{ErrorType, FailureCategory, TaskMode, TaskStatus};
use crate::provider::{self, ProviderCommand};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command as ProcessCommand};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::{sleep, timeout};

const ACP_EXIT_GRACE: Duration = Duration::from_millis(500);

pub(crate) struct AcpProbeResult {
    pub ok: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub error: Option<String>,
    pub timed_out: bool,
    pub duration_ms: u64,
}

struct AcpOutput {
    stdout: Vec<u8>,
    final_text: String,
    saw_output: bool,
    paths: Option<AcpOutputPaths>,
}

struct AcpOutputPaths {
    provider: crate::domain::ProviderKind,
    stdout_path: PathBuf,
    transcript_path: PathBuf,
    redactions: Vec<String>,
    watch_sender: watch::Sender<u64>,
}

struct AcpDialogResult {
    stop_reason: String,
}

struct SpawnedAcp {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    pid: u32,
}

struct AcpTaskRuntime {
    agent_id: String,
    mode: TaskMode,
    command: ProviderCommand,
    agent_dir: PathBuf,
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    output: AcpOutput,
    cancel_rx: oneshot::Receiver<()>,
    stderr_task: Option<tokio::task::JoinHandle<()>>,
}

pub(super) async fn launch_acp_task(
    agent_id: String,
    mode: TaskMode,
    command: ProviderCommand,
    agent_dir: PathBuf,
    tx: mpsc::Sender<ActorCommand>,
    watch_sender: watch::Sender<u64>,
) -> Result<ActiveTask, String> {
    let transcript_path = agent_dir.join("transcript.jsonl");
    let stdout_path = agent_dir.join("stdout.log");
    let stderr_path = agent_dir.join("stderr.log");
    let mut spawned = spawn_acp_process(&command)?;
    let pid = spawned.pid;
    register_active_pid(pid);
    append_transcript_event(
        &transcript_path,
        command.provider,
        "lifecycle",
        "lifecycle",
        "",
        json!({"phase": "spawned", "pid": pid, "profile": command.profile, "transport": "acp"}),
        &command.redactions,
    )
    .await;
    let redactions = super::complete::diagnostic_redactions(&command);
    let stderr_task = spawned.child.stderr.take().map(|stderr| {
        tokio::spawn(drain_log(
            stderr,
            DrainLogContext {
                agent_id: agent_id.clone(),
                path: stderr_path,
                transcript_path: transcript_path.clone(),
                provider: command.provider,
                mode,
                source: "stderr",
                redactions: redactions.clone(),
                watch_sender: watch_sender.clone(),
            },
        ))
    });
    let output = AcpOutput {
        stdout: Vec::new(),
        final_text: String::new(),
        saw_output: false,
        paths: Some(AcpOutputPaths {
            provider: command.provider,
            stdout_path,
            transcript_path: transcript_path.clone(),
            redactions,
            watch_sender,
        }),
    };
    let (cancel_tx, cancel_rx) = oneshot::channel();
    tokio::spawn(async move {
        let completion = run_acp_task(AcpTaskRuntime {
            agent_id,
            mode,
            command,
            agent_dir,
            child: spawned.child,
            stdin: spawned.stdin,
            stdout: spawned.stdout,
            output,
            cancel_rx,
            stderr_task,
        })
        .await;
        unregister_active_pid(pid);
        if tx.send(ActorCommand::Complete(completion)).await.is_err() {
            tracing::error!("[agent-bridge] task manager dropped completion message");
            std::process::abort();
        }
    });
    Ok(ActiveTask {
        pid: Some(pid),
        cancel: Some(cancel_tx),
    })
}

pub(crate) async fn run_acp_probe(command: &ProviderCommand, timeout_ms: u64) -> AcpProbeResult {
    let started = std::time::Instant::now();
    let spawned = match spawn_acp_process(command) {
        Ok(spawned) => spawned,
        Err(error) => {
            return AcpProbeResult {
                ok: false,
                stdout: Vec::new(),
                stderr: Vec::new(),
                error: Some(error),
                timed_out: false,
                duration_ms: started.elapsed().as_millis() as u64,
            };
        }
    };
    let pid = spawned.pid;
    let mut child = spawned.child;
    let stderr_task = child.stderr.take().map(|mut stderr| {
        tokio::spawn(async move {
            let mut bytes = Vec::new();
            let _ = stderr.read_to_end(&mut bytes).await;
            bytes
        })
    });
    let output = AcpOutput {
        stdout: Vec::new(),
        final_text: String::new(),
        saw_output: false,
        paths: None,
    };
    let result = timeout(
        Duration::from_millis(timeout_ms),
        run_acp_dialog(command, spawned.stdin, spawned.stdout, output),
    )
    .await;
    let (ok, stdout, error, timed_out) = match result {
        Ok(Ok((dialog, output))) => (
            dialog.stop_reason != "cancelled" && !output.final_text.trim().is_empty(),
            output.stdout,
            None,
            false,
        ),
        Ok(Err((error, output))) => (false, output.stdout, Some(error), false),
        Err(_) => {
            terminate_child_tree(pid, libc::SIGTERM);
            (
                false,
                Vec::new(),
                Some("ACP probe timed out".to_string()),
                true,
            )
        }
    };
    finish_child(&mut child, pid, !ok).await;
    let stderr = match stderr_task {
        Some(task) => task.await.unwrap_or_default(),
        None => Vec::new(),
    };
    AcpProbeResult {
        ok,
        stdout,
        stderr,
        error,
        timed_out,
        duration_ms: started.elapsed().as_millis() as u64,
    }
}

fn spawn_acp_process(command: &ProviderCommand) -> Result<SpawnedAcp, String> {
    let mut process = ProcessCommand::new(&command.command);
    process
        .args(&command.args)
        .current_dir(&command.cwd)
        .env_clear()
        .envs(provider::provider_env(command_provider_hint(command)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_process_group(&mut process);
    let mut child = process.spawn().map_err(|error| error.to_string())?;
    let pid = child
        .id()
        .ok_or_else(|| "ACP child process did not expose a pid".to_string())?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "ACP child stdin unavailable".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "ACP child stdout unavailable".to_string())?;
    Ok(SpawnedAcp {
        child,
        stdin,
        stdout,
        pid,
    })
}

async fn run_acp_task(runtime: AcpTaskRuntime) -> TaskCompletion {
    let AcpTaskRuntime {
        agent_id,
        mode,
        command,
        agent_dir,
        mut child,
        stdin,
        stdout,
        output,
        cancel_rx,
        mut stderr_task,
    } = runtime;
    let pid = child.id().unwrap_or_default();
    let timeout_seconds = command.timeout_seconds;
    let stderr_path = agent_dir.join("stderr.log");
    let adapter = provider::adapter_for(command.provider);
    let mut denial_scanner = StderrDenialScanner::default();
    let dialog = timeout(
        Duration::from_secs(timeout_seconds as u64),
        run_acp_dialog(&command, stdin, stdout, output),
    );
    tokio::pin!(dialog);
    let mut cancel_rx = cancel_rx;
    let result = loop {
        tokio::select! {
            result = &mut dialog => break result,
            _ = &mut cancel_rx => {
                terminate_child_tree(pid, libc::SIGTERM);
                append_acp_lifecycle(&agent_dir, &command, json!({"phase": "stopped", "transport": "acp"})).await;
                return TaskCompletion {
                    agent_id,
                    status: TaskStatus::Stopped,
                    exit_code: None,
                    signal: Some("SIGTERM".to_string()),
                    error: Some("task stopped".to_string()),
                    error_type: Some(ErrorType::Stopped),
                    diagnostic: None,
                };
            }
            _ = sleep(Duration::from_millis(50)), if adapter.polls_stderr_for_denial() => {
                if denial_scanner.read_appended(&stderr_path).await.is_some()
                    && adapter.detects_fatal_denial(denial_scanner.buffer())
                {
                    terminate_child_tree(pid, libc::SIGTERM);
                    finish_child(&mut child, pid, true).await;
                    let stderr = wait_stderr(stderr_task.take(), &agent_dir).await;
                    let stdout = fs::read(agent_dir.join("stdout.log")).await.unwrap_or_default();
                    return codex_denial_completion(
                        agent_id,
                        &command,
                        timeout_seconds,
                        None,
                        None,
                        &stdout,
                        &stderr,
                    );
                }
            }
        }
    };
    let (status, error, error_type, diagnostic) = match result {
        Ok(Ok((dialog, mut output))) => {
            append_provider_result(&mut output, &dialog.stop_reason, &command).await;
            finish_child(&mut child, pid, false).await;
            let empty_output = output.final_text.trim().is_empty();
            let status = if dialog.stop_reason == "cancelled" {
                TaskStatus::Stopped
            } else if dialog.stop_reason == "refusal" || empty_output {
                TaskStatus::Failed
            } else {
                TaskStatus::Succeeded
            };
            let error_type = (status == TaskStatus::Stopped)
                .then_some(ErrorType::Stopped)
                .or((status == TaskStatus::Failed).then_some(ErrorType::ProviderOutputError));
            let error = empty_output.then(|| "ACP provider returned no final text".to_string());
            let diagnostic = acp_stop_reason_diagnostic(&command, &dialog.stop_reason);
            (status, error, error_type, diagnostic)
        }
        Ok(Err((error, output))) => {
            finish_child(&mut child, pid, true).await;
            let stderr = wait_stderr(stderr_task.take(), &agent_dir).await;
            if provider::adapter_for(command.provider).detects_fatal_denial(&stderr) {
                return codex_denial_completion(
                    agent_id,
                    &command,
                    timeout_seconds,
                    None,
                    None,
                    &output.stdout,
                    &stderr,
                );
            }
            let diagnostic = Some(agent_diagnostic(
                &command,
                FailureCategory::ProviderOutputError,
                timeout_seconds * 1000,
                None,
                None,
                &output.stdout,
                &stderr,
            ));
            (
                TaskStatus::Failed,
                Some(error),
                Some(ErrorType::ProviderOutputError),
                diagnostic,
            )
        }
        Err(_) => {
            terminate_child_tree(pid, libc::SIGTERM);
            finish_child(&mut child, pid, true).await;
            let stderr = wait_stderr(stderr_task.take(), &agent_dir).await;
            if provider::adapter_for(command.provider).detects_fatal_denial(&stderr) {
                return codex_denial_completion(
                    agent_id,
                    &command,
                    timeout_seconds,
                    None,
                    None,
                    &[],
                    &stderr,
                );
            }
            let diagnostic = Some(agent_diagnostic(
                &command,
                FailureCategory::ProviderTimeout,
                timeout_seconds * 1000,
                None,
                None,
                &[],
                &stderr,
            ));
            (
                TaskStatus::Failed,
                Some(format!("task timed out after {}ms", timeout_seconds * 1000)),
                Some(ErrorType::Timeout),
                diagnostic,
            )
        }
    };
    append_acp_lifecycle(
        &agent_dir,
        &command,
        json!({"phase": "exited", "mode": mode, "profile": command.profile, "transport": "acp"}),
    )
    .await;
    TaskCompletion {
        agent_id,
        status,
        exit_code: (status == TaskStatus::Succeeded).then_some(0),
        signal: None,
        error,
        error_type,
        diagnostic,
    }
}

fn acp_stop_reason_diagnostic(command: &ProviderCommand, stop_reason: &str) -> Option<Value> {
    if !matches!(stop_reason, "cancelled" | "refusal") {
        return None;
    }
    Some(json!({
        "failureCategory": FailureCategory::ProviderOutputError.as_str(),
        "provider": command_provider_hint(command).as_str(),
        "launchStrategy": "acp",
        "acpStopReason": stop_reason,
    }))
}

async fn wait_stderr(task: Option<tokio::task::JoinHandle<()>>, agent_dir: &Path) -> Vec<u8> {
    if let Some(task) = task {
        let _ = timeout(super::CHILD_SHUTDOWN_GRACE, task).await;
    }
    fs::read(agent_dir.join("stderr.log"))
        .await
        .unwrap_or_default()
}

async fn run_acp_dialog(
    command: &ProviderCommand,
    mut stdin: ChildStdin,
    stdout: ChildStdout,
    mut output: AcpOutput,
) -> Result<(AcpDialogResult, AcpOutput), (String, AcpOutput)> {
    let Some(prompt) = command.stdin.as_deref() else {
        return Err(("ACP command is missing rendered prompt".to_string(), output));
    };
    let mut reader = BufReader::new(stdout);
    if let Err(error) = write_request(
        &mut stdin,
        1,
        "initialize",
        json!({"protocolVersion": 1, "clientCapabilities": {}}),
    )
    .await
    {
        return Err((error, output));
    }
    if let Err(error) = read_response(&mut reader, &mut stdin, 1, command, &mut output).await {
        return Err((error, output));
    }
    if let Err(error) = write_request(
        &mut stdin,
        2,
        "session/new",
        json!({"cwd": command.cwd.clone(), "mcpServers": []}),
    )
    .await
    {
        return Err((error, output));
    }
    let new_session = match read_response(&mut reader, &mut stdin, 2, command, &mut output).await {
        Ok(result) => result,
        Err(error) => return Err((error, output)),
    };
    let Some(session_id) = new_session.get("sessionId").and_then(Value::as_str) else {
        return Err((
            "ACP session/new response missing sessionId".to_string(),
            output,
        ));
    };
    if let Err(error) = write_request(
        &mut stdin,
        3,
        "session/prompt",
        json!({"sessionId": session_id, "prompt": [{"type": "text", "text": prompt}]}),
    )
    .await
    {
        return Err((error, output));
    }
    let prompt_response =
        match read_response(&mut reader, &mut stdin, 3, command, &mut output).await {
            Ok(result) => result,
            Err(error) => return Err((error, output)),
        };
    let stop_reason = prompt_response
        .get("stopReason")
        .and_then(Value::as_str)
        .unwrap_or("end_turn")
        .to_string();
    Ok((AcpDialogResult { stop_reason }, output))
}

async fn write_request(
    stdin: &mut ChildStdin,
    id: i64,
    method: &str,
    params: Value,
) -> Result<(), String> {
    let message = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
    stdin
        .write_all(message.to_string().as_bytes())
        .await
        .map_err(|error| error.to_string())?;
    stdin
        .write_all(b"\n")
        .await
        .map_err(|error| error.to_string())?;
    stdin.flush().await.map_err(|error| error.to_string())
}

async fn read_response(
    reader: &mut BufReader<ChildStdout>,
    stdin: &mut ChildStdin,
    id: i64,
    command: &ProviderCommand,
    output: &mut AcpOutput,
) -> Result<Value, String> {
    loop {
        let mut line = String::new();
        let bytes = reader
            .read_line(&mut line)
            .await
            .map_err(|error| error.to_string())?;
        if bytes == 0 {
            return Err("ACP child closed stdout before responding".to_string());
        }
        let value: Value = serde_json::from_str(line.trim())
            .map_err(|error| format!("invalid ACP JSON: {error}"))?;
        if let Some(text) = agent_text(&value) {
            append_agent_text(output, command, text).await;
        }
        if value.get("id").and_then(Value::as_i64) == Some(id) {
            if let Some(error) = value.get("error") {
                return Err(format!("ACP request failed: {error}"));
            }
            return Ok(value.get("result").cloned().unwrap_or(Value::Null));
        }
        if value.get("id").is_some() && value.get("method").is_some() {
            write_method_not_found(stdin, value.get("id").cloned().unwrap_or(Value::Null)).await?;
        }
    }
}

async fn write_method_not_found(stdin: &mut ChildStdin, id: Value) -> Result<(), String> {
    let message = json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": -32601, "message": "method not supported by Agent Bridge ACP runner"}
    });
    stdin
        .write_all(message.to_string().as_bytes())
        .await
        .map_err(|error| error.to_string())?;
    stdin
        .write_all(b"\n")
        .await
        .map_err(|error| error.to_string())?;
    stdin.flush().await.map_err(|error| error.to_string())
}

fn agent_text(value: &Value) -> Option<&str> {
    let update = value.get("params")?.get("update")?;
    let kind = update.get("sessionUpdate")?.as_str()?;
    if !matches!(kind, "agent_message_chunk" | "agent_thought_chunk") {
        return None;
    }
    let content = update.get("content")?;
    (content.get("type").and_then(Value::as_str) == Some("text"))
        .then(|| content.get("text").and_then(Value::as_str))
        .flatten()
}

async fn append_agent_text(output: &mut AcpOutput, command: &ProviderCommand, text: &str) {
    if text.is_empty() {
        return;
    }
    output.stdout.extend_from_slice(text.as_bytes());
    output.final_text.push_str(text);
    let Some(paths) = output.paths.as_ref() else {
        return;
    };
    if let Some(parent) = paths.stdout_path.parent() {
        let _ = fs::create_dir_all(parent).await;
    }
    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.stdout_path)
        .await
    {
        let _ = file.write_all(text.as_bytes()).await;
    }
    if !output.saw_output {
        append_transcript_event(
            &paths.transcript_path,
            paths.provider,
            "lifecycle",
            "lifecycle",
            "",
            json!({"phase": "first_output", "source": "stdout", "transport": "acp"}),
            &paths.redactions,
        )
        .await;
        output.saw_output = true;
    }
    append_transcript_event(
        &paths.transcript_path,
        command.provider,
        "stdout",
        "provider_event",
        text,
        json!({"type": "acp_agent_message", "text": text}),
        &paths.redactions,
    )
    .await;
    paths
        .watch_sender
        .send_modify(|version| *version = version.wrapping_add(1));
}

async fn append_provider_result(
    output: &mut AcpOutput,
    stop_reason: &str,
    command: &ProviderCommand,
) {
    let Some(paths) = output.paths.as_ref() else {
        return;
    };
    append_transcript_event(
        &paths.transcript_path,
        command.provider,
        "stdout",
        "provider_result",
        &output.final_text,
        json!({"type": "result", "result": output.final_text, "stopReason": stop_reason}),
        &paths.redactions,
    )
    .await;
    append_transcript_event(
        &paths.transcript_path,
        paths.provider,
        "lifecycle",
        "lifecycle",
        "",
        json!({"phase": "final_output", "source": "stdout", "transport": "acp"}),
        &paths.redactions,
    )
    .await;
    paths
        .watch_sender
        .send_modify(|version| *version = version.wrapping_add(1));
}

async fn append_acp_lifecycle(agent_dir: &Path, command: &ProviderCommand, payload: Value) {
    append_transcript_event(
        &agent_dir.join("transcript.jsonl"),
        command.provider,
        "lifecycle",
        "lifecycle",
        "",
        payload,
        &command.redactions,
    )
    .await;
}

async fn finish_child(child: &mut Child, pid: u32, terminate: bool) {
    if terminate {
        terminate_child_tree(pid, libc::SIGTERM);
    }
    if timeout(ACP_EXIT_GRACE, child.wait()).await.is_ok() {
        return;
    }
    terminate_child_tree(pid, libc::SIGTERM);
    if timeout(super::CHILD_SHUTDOWN_GRACE, child.wait())
        .await
        .is_ok()
    {
        return;
    }
    terminate_child_tree(pid, libc::SIGKILL);
    let _ = timeout(super::SIGKILL_REAP_GRACE, child.wait()).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{LaunchProfile, ProviderKind};
    use std::collections::BTreeMap;

    fn command() -> ProviderCommand {
        ProviderCommand {
            provider: ProviderKind::Claude,
            command_kind: Some("acp".to_string()),
            claude_host: None,
            command: "claude-agent".to_string(),
            args: Vec::new(),
            stdin: None,
            redactions: Vec::new(),
            cwd: ".".to_string(),
            timeout_seconds: 5,
            env: BTreeMap::new(),
            profile: LaunchProfile::Bridge,
            prompt_strategy: "bridge".to_string(),
            profile_diagnostics: Value::Null,
        }
    }

    #[test]
    fn semantic_stop_reason_is_preserved_in_diagnostic() {
        for stop_reason in ["refusal", "cancelled"] {
            let diagnostic = acp_stop_reason_diagnostic(&command(), stop_reason).unwrap();

            assert_eq!(diagnostic["acpStopReason"], stop_reason);
            assert_eq!(diagnostic["failureCategory"], "provider_output_error");
            assert_eq!(diagnostic["provider"], "claude");
        }
    }

    #[test]
    fn ordinary_end_turn_has_no_stop_reason_diagnostic() {
        assert!(acp_stop_reason_diagnostic(&command(), "end_turn").is_none());
    }
}
