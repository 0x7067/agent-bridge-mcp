use super::complete::{
    append_transcript_event, command_provider_hint, complete_host_response, diagnostic_redactions,
    run_git, run_git_stdout,
};
use super::registry::now_iso;
use super::review::transition_status;
use super::supervision::{
    ChildIoDrains, DrainLogContext, configure_child_process_group, drain_log, register_active_pid,
    unregister_active_pid, wait_for_child,
};
use super::{ActiveTask, ActorCommand, MAX_PROMPT_BYTES, TaskCompletion, TaskRecord};
use crate::domain::{
    ErrorType, FailureCategory, LaunchProfile, ProviderKind, TaskMode, TaskStatus, WorktreeName,
};
use crate::provider::{self, ProviderCommand};
use crate::tools::TaskPreviewInput;
use serde_json::{Value, json};
use std::env;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::process::Command as ProcessCommand;
use tokio::sync::{mpsc, oneshot, watch};

pub(super) fn default_launch_profile() -> LaunchProfile {
    LaunchProfile::Bridge
}

/// Applies a `launch_task` outcome to a freshly built record: on success marks it
/// running and returns the active handle for the caller to track; on failure marks
/// it failed and removes any worktree created before the launch attempt so it is
/// not left orphaned. `cwd` is the pre-worktree directory used for git cleanup.
pub(super) async fn apply_launch_outcome(
    record: &mut TaskRecord,
    outcome: Result<ActiveTask, String>,
    cwd: &str,
) -> Result<Option<ActiveTask>, String> {
    match outcome {
        Ok(active) => {
            record.pid = active.pid;
            transition_status(record, TaskStatus::Running)?;
            record.started_at = Some(now_iso());
            record.updated_at = record.started_at.clone().unwrap();
            Ok(Some(active))
        }
        Err(error) => {
            transition_status(record, TaskStatus::Failed)?;
            record.error = Some(error);
            record.error_type = Some(ErrorType::ProviderStartError);
            record.completed_at = Some(now_iso());
            record.updated_at = record.completed_at.clone().unwrap();
            if record.worktree_managed
                && let Some(worktree_path) = record.worktree_path.clone()
            {
                match run_git(&["worktree", "remove", "-f", &worktree_path], cwd).await {
                    Ok(_) => {
                        record.worktree_managed = false;
                    }
                    Err(cleanup_error) => {
                        record.diagnostic = Some(json!({
                            "failureCategory": FailureCategory::WorktreeCleanupFailed.as_str(),
                            "message": format!(
                                "failed to remove worktree after launch failure: {cleanup_error}"
                            ),
                            "worktreePath": worktree_path,
                        }));
                    }
                }
            }
            Ok(None)
        }
    }
}

/// Spawns the Claude owned-host-runner task on the background runtime and returns
/// a cancellable `ActiveTask`. The spawned task forwards its completion to the
/// manager, aborting the process if that channel is gone.
pub(super) fn launch_host_runner_task(
    agent_id: String,
    command: ProviderCommand,
    claude_command: crate::claude_host::ClaudeHostCommand,
    socket_path: PathBuf,
    agent_dir: PathBuf,
    tx: mpsc::Sender<ActorCommand>,
) -> ActiveTask {
    let (cancel_tx, cancel_rx) = oneshot::channel();
    tokio::spawn(async move {
        let completion = run_host_task(
            agent_id,
            command,
            claude_command,
            socket_path,
            agent_dir,
            cancel_rx,
        )
        .await;
        if tx.send(ActorCommand::Complete(completion)).await.is_err() {
            tracing::error!("[agent-bridge] task manager dropped completion message");
            std::process::abort();
        }
    });
    ActiveTask {
        pid: None,
        cancel: Some(cancel_tx),
    }
}

#[tracing::instrument(
    name = "launch_child",
    skip(command, agent_dir, tx, watch_sender),
    fields(
        agent_id = %agent_id,
        provider = tracing::field::Empty,
        mode = tracing::field::Empty,
        task_status = "queued"
    )
)]
pub(super) async fn launch_task(
    agent_id: String,
    mode: TaskMode,
    command: ProviderCommand,
    agent_dir: PathBuf,
    tx: mpsc::Sender<ActorCommand>,
    watch_sender: watch::Sender<u64>,
) -> Result<ActiveTask, String> {
    let span = tracing::Span::current();
    span.record(
        "provider",
        tracing::field::display(command.provider.as_str()),
    );
    span.record("mode", tracing::field::display(mode.as_str()));
    if command.is_acp() {
        return super::acp::launch_acp_task(agent_id, mode, command, agent_dir, tx, watch_sender)
            .await;
    }
    if command.provider == ProviderKind::Claude
        && let (Some(socket_path), Some(claude_command)) = (
            crate::claude_host::socket_path_from_env(),
            command.claude_host.clone(),
        )
    {
        return Ok(launch_host_runner_task(
            agent_id,
            command,
            claude_command,
            socket_path,
            agent_dir,
            tx,
        ));
    }
    let stdout_path = agent_dir.join("stdout.log");
    let stderr_path = agent_dir.join("stderr.log");
    let transcript_path = agent_dir.join("transcript.jsonl");
    let mut process = ProcessCommand::new(&command.command);
    process
        .args(&command.args)
        .current_dir(&command.cwd)
        .env_clear()
        .envs(provider::provider_env(command_provider_hint(&command)))
        .stdin(if command.stdin.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    configure_child_process_group(&mut process);
    let mut child = process.spawn().map_err(|error| error.to_string())?;
    if let Some(stdin) = command.stdin.clone()
        && let Some(mut child_stdin) = child.stdin.take()
    {
        tokio::spawn(async move {
            let _ = child_stdin.write_all(stdin.as_bytes()).await;
        });
    }
    let pid = child
        .id()
        .ok_or_else(|| "provider process did not expose a pid".to_string())?;
    register_active_pid(pid);
    append_transcript_event(
        &transcript_path,
        command.provider,
        "lifecycle",
        "lifecycle",
        "",
        json!({"phase": "spawned", "pid": pid, "profile": command.profile}),
        &command.redactions,
    )
    .await;
    let redactions = diagnostic_redactions(&command);
    let drains = ChildIoDrains {
        stdout: child.stdout.take().map(|stdout| {
            tokio::spawn(drain_log(
                stdout,
                DrainLogContext {
                    agent_id: agent_id.clone(),
                    path: stdout_path,
                    transcript_path: transcript_path.clone(),
                    provider: command.provider,
                    mode,
                    source: "stdout",
                    redactions: redactions.clone(),
                    watch_sender: watch_sender.clone(),
                },
            ))
        }),
        stderr: child.stderr.take().map(|stderr| {
            tokio::spawn(drain_log(
                stderr,
                DrainLogContext {
                    agent_id: agent_id.clone(),
                    path: stderr_path,
                    transcript_path,
                    provider: command.provider,
                    mode,
                    source: "stderr",
                    redactions,
                    watch_sender,
                },
            ))
        }),
    };
    tokio::spawn(async move {
        let completion =
            wait_for_child(agent_id, pid, child, mode, command, agent_dir, drains).await;
        unregister_active_pid(pid);
        if tx.send(ActorCommand::Complete(completion)).await.is_err() {
            tracing::error!("[agent-bridge] task manager dropped completion message");
            std::process::abort();
        }
    });
    Ok(ActiveTask {
        pid: Some(pid),
        cancel: None,
    })
}

pub(super) async fn run_host_task(
    agent_id: String,
    command: ProviderCommand,
    claude_command: crate::claude_host::ClaudeHostCommand,
    socket_path: PathBuf,
    agent_dir: PathBuf,
    cancel_rx: oneshot::Receiver<()>,
) -> TaskCompletion {
    let result = tokio::select! {
        result = crate::claude_host::run_claude(&socket_path, &claude_command) => result,
        _ = cancel_rx => {
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
    };
    match result {
        Ok(response) if response.ok => {
            complete_host_response(agent_id, command, agent_dir, response).await
        }
        Ok(response) => TaskCompletion {
            agent_id,
            status: TaskStatus::Failed,
            exit_code: None,
            signal: None,
            error: response
                .error
                .map(|error| error.message)
                .or_else(|| Some("host runner failed".to_string())),
            error_type: Some(ErrorType::ProviderStartError),
            diagnostic: None,
        },
        Err(error) => TaskCompletion {
            agent_id,
            status: TaskStatus::Failed,
            exit_code: None,
            signal: None,
            error: Some(error),
            error_type: Some(ErrorType::ProviderStartError),
            diagnostic: None,
        },
    }
}

pub(super) async fn create_worktree(
    state_dir: &Path,
    cwd: &str,
    provider: ProviderKind,
    mode: crate::domain::TaskMode,
    agent_id: &str,
    worktree_name: Option<&str>,
) -> Result<String, String> {
    let root = run_git_stdout(&["rev-parse", "--show-toplevel"], cwd)
        .await?
        .trim()
        .to_string();
    let base_name = worktree_name.map(str::to_string).unwrap_or_else(|| {
        format!(
            "{}-{}-{}",
            provider.as_str(),
            mode.as_str(),
            &agent_id[agent_id.len() - 8..]
        )
    });
    let branch_name = format!("agent-bridge/{base_name}");
    let worktree_root = state_dir.join("worktrees");
    fs::create_dir_all(&worktree_root)
        .await
        .map_err(|error| error.to_string())?;
    let worktree_path = worktree_root.join(base_name);
    let worktree_path_string = worktree_path.display().to_string();
    run_git(
        &["worktree", "add", "-b", &branch_name, &worktree_path_string],
        &root,
    )
    .await?;
    Ok(worktree_path_string)
}

pub(super) fn validate_spawn_arguments(arguments: Value) -> Result<TaskPreviewInput, String> {
    let input: TaskPreviewInput =
        serde_json::from_value(arguments).map_err(|error| error.to_string())?;
    if input.prompt.is_empty() {
        return Err("prompt is required".to_string());
    }
    if input.prompt.len() > MAX_PROMPT_BYTES {
        return Err(format!("prompt exceeds {MAX_PROMPT_BYTES} bytes"));
    }
    if let Some(name) = input.worktree_name.as_deref() {
        WorktreeName::new(name)?;
    }
    Ok(input)
}

pub(super) fn safe_cwd(cwd: Option<&str>) -> Result<String, String> {
    let default_cwd = env::current_dir().map_err(|error| error.to_string())?;
    let cwd = cwd.map(PathBuf::from).unwrap_or(default_cwd);
    if cwd
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err("cwd must not contain .. segments".to_string());
    }
    let real_cwd = cwd.canonicalize().map_err(|error| error.to_string())?;
    let workspace_roots = configured_workspace_roots()?;
    if !workspace_roots
        .iter()
        .any(|root| real_cwd == *root || real_cwd.strip_prefix(root).is_ok())
    {
        return Err(format!(
            "cwd is outside configured workspaces: {}",
            workspace_roots
                .iter()
                .map(|root| root.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    Ok(real_cwd.display().to_string())
}

pub(super) fn configured_workspace_roots() -> Result<Vec<PathBuf>, String> {
    crate::config::runtime_workspace_roots()
}
