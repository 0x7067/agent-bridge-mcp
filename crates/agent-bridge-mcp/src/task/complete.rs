use super::registry::{cap_string, now_iso};
use super::supervision::{append_stream_transcript, signal_name};
use super::{MAX_LOG_BYTES, TaskCompletion};
use crate::domain::{ErrorType, FailureCategory, ProviderKind, TaskStatus};
use crate::provider::{self, ProviderCommand};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::process::Command as ProcessCommand;

pub(super) async fn complete_host_response(
    agent_id: String,
    command: ProviderCommand,
    agent_dir: PathBuf,
    response: crate::claude_host::HostResponse,
) -> TaskCompletion {
    let Some(crate::claude_host::HostResult::Run(run)) = response.result else {
        return TaskCompletion {
            agent_id,
            status: TaskStatus::Failed,
            exit_code: None,
            signal: None,
            error: Some("host runner returned unexpected response".to_string()),
            error_type: Some(ErrorType::ProviderOutputError),
            diagnostic: None,
        };
    };
    let crate::claude_host::HostRunResult {
        status,
        exit_code,
        signal,
        stdout,
        stderr,
        failure_category,
        result,
        transcript,
        stop,
        stop_failure,
        ..
    } = *run;
    let stdout_bytes = stdout.as_bytes().to_vec();
    let stderr_bytes = stderr.as_bytes().to_vec();
    let _ = fs::write(agent_dir.join("stdout.log"), &stdout_bytes).await;
    let _ = fs::write(agent_dir.join("stderr.log"), &stderr_bytes).await;
    let transcript_path = agent_dir.join("transcript.jsonl");
    let redactions = diagnostic_redactions(&command);
    append_transcript_event(
        &transcript_path,
        command.provider,
        "lifecycle",
        "lifecycle",
        "",
        json!({
            "phase": "host_response",
            "profile": command.profile,
            "status": status,
            "transcript": transcript,
            "stop": stop,
            "stopFailure": stop_failure
        }),
        &redactions,
    )
    .await;
    if let Some(success) = result.as_ref() {
        append_transcript_event(
            &transcript_path,
            command.provider,
            "host_runner",
            "provider_result",
            "",
            json!({
                "type": "result",
                "result": success.final_text,
                "source": success.source,
                "sessionId": success.session_id
            }),
            &redactions,
        )
        .await;
    }
    append_stream_transcript(
        &transcript_path,
        command.provider,
        "stdout",
        &stdout,
        &redactions,
    )
    .await;
    append_stream_transcript(
        &transcript_path,
        command.provider,
        "stderr",
        &stderr,
        &redactions,
    )
    .await;
    host_completion(
        agent_id,
        &command,
        HostOutcome {
            failure_category,
            result_present: result.is_some(),
            exit_code,
            signal,
            stdout_bytes: &stdout_bytes,
            stderr_bytes: &stderr_bytes,
        },
    )
}

/// The exit evidence from a finished host-runner task, used to build its
/// `TaskCompletion`.
pub(super) struct HostOutcome<'a> {
    failure_category: Option<crate::domain::FailureCategory>,
    result_present: bool,
    exit_code: Option<i32>,
    signal: Option<String>,
    stdout_bytes: &'a [u8],
    stderr_bytes: &'a [u8],
}

/// Builds the `TaskCompletion` for a finished host-runner task: success when there
/// is a result and no failure category, otherwise a failure carrying the category,
/// error type, and a diagnostic snapshot.
pub(super) fn host_completion(
    agent_id: String,
    command: &ProviderCommand,
    outcome: HostOutcome,
) -> TaskCompletion {
    let HostOutcome {
        failure_category,
        result_present,
        exit_code,
        signal,
        stdout_bytes,
        stderr_bytes,
    } = outcome;
    if failure_category.is_none() && result_present {
        return TaskCompletion {
            agent_id,
            status: TaskStatus::Succeeded,
            exit_code,
            signal,
            error: None,
            error_type: None,
            diagnostic: None,
        };
    }
    let category = failure_category.unwrap_or(crate::domain::FailureCategory::ProviderExitError);
    TaskCompletion {
        agent_id,
        status: TaskStatus::Failed,
        exit_code,
        signal: signal.clone(),
        error: Some(category.to_string()),
        error_type: Some(
            if category == crate::domain::FailureCategory::ProviderTimeout {
                ErrorType::Timeout
            } else {
                ErrorType::ProviderExitError
            },
        ),
        diagnostic: Some(agent_diagnostic(
            command,
            category,
            command.timeout_seconds * 1000,
            exit_code,
            signal,
            stdout_bytes,
            stderr_bytes,
        )),
    }
}

/// Maps a finished direct-child exit into a `TaskCompletion`, applying adapter
/// denial/parseability checks on success and shaping timeout/exit failures. Reads
/// the captured stdout/stderr logs from `agent_dir` as needed.
pub(super) fn classify_completion(
    agent_id: String,
    command: &ProviderCommand,
    agent_dir: &Path,
    timeout_seconds: i64,
    output: Result<std::process::ExitStatus, String>,
    timed_out: bool,
    fatal_denial: bool,
) -> TaskCompletion {
    match output {
        Ok(status) if status.success() => classify_success_exit(
            agent_id,
            command,
            agent_dir,
            timeout_seconds,
            status,
            fatal_denial,
        ),
        Ok(status) => classify_failure_exit(
            agent_id,
            command,
            agent_dir,
            timeout_seconds,
            status,
            timed_out,
            fatal_denial,
        ),
        Err(error) => TaskCompletion {
            agent_id,
            status: TaskStatus::Failed,
            exit_code: None,
            signal: None,
            error: Some(error),
            error_type: Some(ErrorType::ProviderExitError),
            diagnostic: None,
        },
    }
}

/// Classifies a process that exited 0: a fatal denial or unparseable output still
/// becomes a failure for adapters that enforce those checks; otherwise success.
pub(super) fn classify_success_exit(
    agent_id: String,
    command: &ProviderCommand,
    agent_dir: &Path,
    timeout_seconds: i64,
    status: std::process::ExitStatus,
    fatal_denial: bool,
) -> TaskCompletion {
    let adapter = provider::adapter_for(command.provider);
    if adapter.polls_stderr_for_denial() || fatal_denial {
        let stdout = std::fs::read(agent_dir.join("stdout.log")).unwrap_or_default();
        let stderr = std::fs::read(agent_dir.join("stderr.log")).unwrap_or_default();
        if fatal_denial || adapter.detects_fatal_denial(&stderr) {
            return codex_denial_completion(
                agent_id,
                command,
                timeout_seconds,
                status.code(),
                signal_name(&status),
                &stdout,
                &stderr,
            );
        }
    }
    if adapter.enforces_output_parseable() {
        let stdout = std::fs::read(agent_dir.join("stdout.log")).unwrap_or_default();
        let stderr = std::fs::read(agent_dir.join("stderr.log")).unwrap_or_default();
        if !adapter.output_is_acceptable(&stdout) {
            return TaskCompletion {
                agent_id,
                status: TaskStatus::Failed,
                exit_code: status.code(),
                signal: signal_name(&status),
                error: Some("claude provider output was not parseable".to_string()),
                error_type: Some(ErrorType::ProviderOutputError),
                diagnostic: Some(agent_diagnostic(
                    command,
                    FailureCategory::ProviderOutputError,
                    timeout_seconds * 1000,
                    status.code(),
                    signal_name(&status),
                    &stdout,
                    &stderr,
                )),
            };
        }
    }
    TaskCompletion {
        agent_id,
        status: TaskStatus::Succeeded,
        exit_code: status.code(),
        signal: signal_name(&status),
        error: None,
        error_type: None,
        diagnostic: None,
    }
}

/// Classifies a process that exited non-zero: a fatal denial maps to the denial
/// completion; otherwise a timeout or plain exit failure with a diagnostic.
pub(super) fn classify_failure_exit(
    agent_id: String,
    command: &ProviderCommand,
    agent_dir: &Path,
    timeout_seconds: i64,
    status: std::process::ExitStatus,
    timed_out: bool,
    fatal_denial: bool,
) -> TaskCompletion {
    let signal = signal_name(&status);
    let stdout = std::fs::read(agent_dir.join("stdout.log")).unwrap_or_default();
    let stderr = std::fs::read(agent_dir.join("stderr.log")).unwrap_or_default();
    if fatal_denial || provider::adapter_for(command.provider).detects_fatal_denial(&stderr) {
        return codex_denial_completion(
            agent_id,
            command,
            timeout_seconds,
            status.code(),
            signal,
            &stdout,
            &stderr,
        );
    }
    TaskCompletion {
        agent_id,
        status: TaskStatus::Failed,
        exit_code: status.code(),
        signal: signal.clone(),
        error: if timed_out {
            Some(format!("task timed out after {}ms", timeout_seconds * 1000))
        } else {
            Some(format!(
                "command exited with code {}",
                status.code().unwrap_or(-1)
            ))
        },
        error_type: Some(if timed_out {
            ErrorType::Timeout
        } else {
            ErrorType::ProviderExitError
        }),
        diagnostic: Some(agent_diagnostic(
            command,
            if timed_out {
                FailureCategory::ProviderTimeout
            } else {
                FailureCategory::ProviderExitError
            },
            timeout_seconds * 1000,
            status.code(),
            signal,
            &stdout,
            &stderr,
        )),
    }
}

pub(super) fn codex_denial_completion(
    agent_id: String,
    command: &ProviderCommand,
    timeout_seconds: i64,
    exit_code: Option<i32>,
    signal: Option<String>,
    stdout: &[u8],
    stderr: &[u8],
) -> TaskCompletion {
    TaskCompletion {
        agent_id,
        status: TaskStatus::Failed,
        exit_code,
        signal: signal.clone(),
        error: Some("Codex sandbox or approval denied".to_string()),
        error_type: Some(ErrorType::CodexSandboxDenied),
        diagnostic: Some(agent_diagnostic(
            command,
            FailureCategory::ProviderSandboxDenied,
            timeout_seconds * 1000,
            exit_code,
            signal,
            stdout,
            stderr,
        )),
    }
}

pub(super) fn agent_diagnostic(
    command: &ProviderCommand,
    failure_category: FailureCategory,
    timeout_ms: i64,
    exit_code: Option<i32>,
    signal: Option<String>,
    stdout: &[u8],
    stderr: &[u8],
) -> Value {
    let redactions = diagnostic_redactions(command);
    json!({
        "failureCategory": failure_category.as_str(),
        "provider": command_provider_hint(command).as_str(),
        "commandKind": command_kind(command),
        "commandPath": command_path(command),
        "launchStrategy": launch_strategy(command.provider),
        "startupVerified": false,
        "timeoutMs": timeout_ms,
        "exitCode": exit_code,
        "signal": signal,
        "stdoutExcerpt": diagnostic_excerpt(stdout, &redactions),
        "stderrExcerpt": diagnostic_excerpt(stderr, &redactions)
    })
}

pub(super) fn diagnostic_redactions(command: &ProviderCommand) -> Vec<String> {
    let mut redactions = command.redactions.clone();
    redactions.extend(provider_env_redactions(command.provider));
    redactions
}

pub(super) fn provider_env_redactions(provider: ProviderKind) -> Vec<String> {
    provider::provider_env(provider)
        .into_iter()
        .filter(|(key, _)| key.contains("KEY") || key.contains("TOKEN") || key.contains("SECRET"))
        .map(|(_, value)| value)
        .filter(|value| !value.is_empty())
        .collect()
}

pub(super) fn command_kind(command: &ProviderCommand) -> String {
    command
        .command_kind
        .as_deref()
        .unwrap_or(command.provider.as_str())
        .to_string()
}

pub(super) fn launch_strategy(provider: ProviderKind) -> &'static str {
    if provider != ProviderKind::Claude {
        return "direct";
    }
    if crate::claude_host::socket_path_from_env().is_some() {
        "host_runner"
    } else {
        "host_runner_required"
    }
}

pub(super) fn command_path(command: &ProviderCommand) -> String {
    if command.command == "/bin/zsh" {
        return command
            .args
            .get(3)
            .cloned()
            .unwrap_or_else(|| command.command.clone());
    }
    command.command.clone()
}

pub(super) fn diagnostic_excerpt(bytes: &[u8], redactions: &[String]) -> String {
    const EXCERPT_BYTES: usize = 2048;
    let capped = &bytes[..bytes.len().min(EXCERPT_BYTES)];
    let mut text = String::from_utf8_lossy(capped).to_string();
    for value in redactions {
        if !value.is_empty() {
            text = text.replace(value, "<prompt redacted>");
            for token in value.split_whitespace().filter(|token| token.len() >= 8) {
                text = text.replace(token, "<prompt redacted>");
            }
        }
    }
    text
}

pub(super) async fn append_transcript_event(
    transcript_path: &Path,
    provider: ProviderKind,
    source: &str,
    kind: &str,
    raw: &str,
    parsed: Value,
    redactions: &[String],
) {
    let event = json!({
        "ts": now_iso(),
        "source": source,
        "provider": provider,
        "kind": kind,
        "raw": redact_text(raw, redactions),
        "parsed": redact_value(parsed, redactions),
        "redacted": redactions.iter().any(|value| !value.is_empty() && raw.contains(value))
    });
    if let Some(parent) = transcript_path.parent() {
        let _ = fs::create_dir_all(parent).await;
    }
    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(transcript_path)
        .await
    {
        let mut line = event.to_string();
        line.push('\n');
        let _ = file.write_all(line.as_bytes()).await;
    }
}

pub(super) fn redact_value(value: Value, redactions: &[String]) -> Value {
    match value {
        Value::String(text) => Value::String(redact_text(&text, redactions)),
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|value| redact_value(value, redactions))
                .collect(),
        ),
        Value::Object(object) => Value::Object(
            object
                .into_iter()
                .map(|(key, value)| (key, redact_value(value, redactions)))
                .collect(),
        ),
        other => other,
    }
}

pub(super) fn redact_text(text: &str, redactions: &[String]) -> String {
    let mut output = text.to_string();
    for redaction in redactions.iter().filter(|value| !value.is_empty()) {
        output = output.replace(redaction, "<redacted>");
    }
    output
}

pub(super) struct GitSnapshot {
    pub git_status: String,
    pub git_diff: String,
    pub changed_files: Vec<String>,
}

pub(super) async fn git_snapshot(cwd: &str) -> GitSnapshot {
    let git_status = run_git_stdout(&["status", "--short"], cwd)
        .await
        .unwrap_or_default();
    let git_diff = run_git_stdout(&["diff", "--"], cwd)
        .await
        .unwrap_or_default();
    let changed_files = run_git_stdout(&["diff", "--name-only"], cwd)
        .await
        .map(|text| {
            text.lines()
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    GitSnapshot {
        git_status: cap_string(git_status, MAX_LOG_BYTES),
        git_diff: cap_string(git_diff, MAX_LOG_BYTES),
        changed_files,
    }
}

pub(super) async fn run_git_stdout(args: &[&str], cwd: &str) -> Result<String, String> {
    let output = git_command(args, cwd).await?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub(super) async fn run_git(args: &[&str], cwd: &str) -> Result<(), String> {
    let _ = git_command(args, cwd).await?;
    Ok(())
}

pub(super) async fn git_command(args: &[&str], cwd: &str) -> Result<std::process::Output, String> {
    let output = ProcessCommand::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .await
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(output)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Err(if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("git {} failed", args.join(" "))
        })
    }
}

pub(super) fn command_provider_hint(command: &ProviderCommand) -> ProviderKind {
    command.provider
}
