use crate::domain::{
    ErrorType, Isolation, ProviderKind, TaskPhase, TaskStatus, TimeoutSeconds, WorktreeName,
};
use crate::provider::{self, ProviderCommand, ProviderTask};
use crate::tools::TaskPreviewInput;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command as ProcessCommand;
use tokio::sync::{OnceCell, mpsc, oneshot};
use tokio::time::{Duration, Instant, sleep, timeout};
use uuid::Uuid;

const MAX_PROMPT_BYTES: usize = 100 * 1024;
const MAX_LOG_BYTES: usize = 1024 * 1024;
const MAX_WAIT_MS: i64 = 60_000;
const ACTOR_BUFFER: usize = 128;
const CHILD_SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

static MANAGER: OnceCell<TaskManagerHandle> = OnceCell::const_new();

#[derive(Clone)]
pub struct TaskManagerHandle {
    tx: mpsc::Sender<ActorCommand>,
}

impl TaskManagerHandle {
    pub async fn from_env() -> Result<Self, String> {
        MANAGER
            .get_or_try_init(|| async {
                let state_dir = expand_home(
                    env::var("AGENT_BRIDGE_STATE_DIR")
                        .unwrap_or_else(|_| "~/.agent-bridge-mcp/state".to_string())
                        .as_str(),
                );
                Self::start(state_dir).await
            })
            .await
            .cloned()
    }

    async fn start(state_dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(state_dir.join("tasks"))
            .await
            .map_err(|error| error.to_string())?;
        let mut registry = load_registry(&state_dir).await?;
        let mut changed = false;
        for task in registry.tasks.values_mut() {
            if matches!(task.status, TaskStatus::Queued | TaskStatus::Running) {
                transition_status(task, TaskStatus::FailedStale)?;
                task.error = Some(
                    "task was running when the MCP server restarted; resume is not supported in v1"
                        .to_string(),
                );
                task.error_type = Some(ErrorType::Stale);
                task.updated_at = now_iso();
                changed = true;
            }
        }
        if changed {
            save_registry(&state_dir, &registry).await?;
        }

        let (tx, rx) = mpsc::channel(ACTOR_BUFFER);
        let actor = TaskActor {
            state_dir,
            registry,
            active: BTreeMap::new(),
            tx: tx.clone(),
        };
        let join = tokio::spawn(actor.run(rx));
        tokio::spawn(async move {
            if let Err(error) = join.await {
                eprintln!("[agent-bridge] task actor failed: {error}");
                std::process::abort();
            }
        });
        Ok(Self { tx })
    }

    pub async fn spawn(&self, arguments: Value) -> Result<Value, String> {
        self.request(|reply| ActorCommand::Spawn(arguments, reply))
            .await
    }

    pub async fn list(&self) -> Result<Value, String> {
        self.request(ActorCommand::List).await
    }

    pub async fn status(&self, task_id: String) -> Result<Value, String> {
        self.request(|reply| ActorCommand::Status(task_id, reply))
            .await
    }

    pub async fn wait(&self, task_id: String, timeout_ms: Option<i64>) -> Result<Value, String> {
        let deadline = Instant::now() + Duration::from_millis(normalize_wait_ms(timeout_ms) as u64);
        loop {
            let status = self.status(task_id.clone()).await?;
            if status["isFinal"].as_bool().unwrap_or(false) {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                let mut status = status;
                status["timedOut"] = json!(true);
                return Ok(status);
            }
            sleep(Duration::from_millis(50)).await;
        }
    }

    pub async fn logs(
        &self,
        task_id: String,
        max_bytes: Option<i64>,
        stdout_line: Option<usize>,
        stderr_line: Option<usize>,
    ) -> Result<Value, String> {
        let task: TaskRecord = self
            .request(|reply| ActorCommand::Get(task_id.clone(), reply))
            .await?;
        let stdout = read_capped_file(
            &PathBuf::from(&task.task_dir).join("stdout.log"),
            normalize_max_bytes(max_bytes),
        )
        .await?;
        let stderr = read_capped_file(
            &PathBuf::from(&task.task_dir).join("stderr.log"),
            normalize_max_bytes(max_bytes),
        )
        .await?;
        let stdout_lines = slice_lines(&stdout.text, stdout_line.unwrap_or(0));
        let stderr_lines = slice_lines(&stderr.text, stderr_line.unwrap_or(0));
        Ok(json!({
            "taskId": task_id,
            "status": task.status,
            "stdout": stdout_lines.text,
            "stderr": stderr_lines.text,
            "stdoutTruncated": stdout.truncated,
            "stderrTruncated": stderr.truncated,
            "nextStdoutLine": stdout_lines.next_line,
            "nextStderrLine": stderr_lines.next_line
        }))
    }

    pub async fn result(&self, task_id: String, max_bytes: Option<i64>) -> Result<Value, String> {
        let task: TaskRecord = self
            .request(|reply| ActorCommand::Get(task_id.clone(), reply))
            .await?;
        let logs = self.logs(task_id, max_bytes, None, None).await?;
        let mut public = public_task(&task);
        public["exitCode"] = task.exit_code.map_or(Value::Null, Value::from);
        public["signal"] = task.signal.clone().map_or(Value::Null, Value::from);
        public["error"] = task.error.clone().map_or(Value::Null, Value::from);
        public["stdout"] = logs["stdout"].clone();
        public["stderr"] = logs["stderr"].clone();
        public["stdoutTruncated"] = logs["stdoutTruncated"].clone();
        public["stderrTruncated"] = logs["stderrTruncated"].clone();
        public["diagnostic"] = task.diagnostic.clone().unwrap_or(Value::Null);
        public["gitStatus"] = Value::String(task.git_status.clone().unwrap_or_default());
        public["gitDiff"] = Value::String(task.git_diff.clone().unwrap_or_default());
        public["changedFiles"] = Value::Array(
            task.changed_files
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(Value::String)
                .collect(),
        );
        public["reviewPacket"] = review_packet(
            &task,
            logs["stdoutTruncated"].as_bool().unwrap_or(false),
            logs["stderrTruncated"].as_bool().unwrap_or(false),
        );
        Ok(public)
    }

    pub async fn stop(&self, task_id: String) -> Result<Value, String> {
        self.request(|reply| ActorCommand::Stop(task_id, reply))
            .await
    }

    pub async fn remove(&self, task_id: String) -> Result<Value, String> {
        self.request(|reply| ActorCommand::Remove(task_id, reply))
            .await
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        self.request(ActorCommand::Shutdown).await
    }

    async fn request<T>(
        &self,
        command: impl FnOnce(oneshot::Sender<Result<T, String>>) -> ActorCommand,
    ) -> Result<T, String> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(command(reply))
            .await
            .map_err(|_| "task manager is unavailable".to_string())?;
        rx.await
            .map_err(|_| "task manager dropped response".to_string())?
    }
}

enum ActorCommand {
    Spawn(Value, oneshot::Sender<Result<Value, String>>),
    List(oneshot::Sender<Result<Value, String>>),
    Status(String, oneshot::Sender<Result<Value, String>>),
    Get(String, oneshot::Sender<Result<TaskRecord, String>>),
    Stop(String, oneshot::Sender<Result<Value, String>>),
    Remove(String, oneshot::Sender<Result<Value, String>>),
    Shutdown(oneshot::Sender<Result<(), String>>),
    Complete(TaskCompletion),
}

struct TaskActor {
    state_dir: PathBuf,
    registry: Registry,
    active: BTreeMap<String, ActiveTask>,
    tx: mpsc::Sender<ActorCommand>,
}

impl TaskActor {
    async fn run(mut self, mut rx: mpsc::Receiver<ActorCommand>) {
        while let Some(command) = rx.recv().await {
            match command {
                ActorCommand::Spawn(arguments, reply) => {
                    let result = self.spawn(arguments).await;
                    let _ = reply.send(result);
                }
                ActorCommand::List(reply) => {
                    let tasks: Vec<Value> = self
                        .registry
                        .tasks
                        .values()
                        .filter(|task| task.status != TaskStatus::Removed)
                        .map(public_task)
                        .collect();
                    let _ = reply.send(Ok(json!({ "tasks": tasks })));
                }
                ActorCommand::Status(task_id, reply) => {
                    let result = self.require_task(&task_id).map(public_task);
                    let _ = reply.send(result);
                }
                ActorCommand::Get(task_id, reply) => {
                    let result = self.require_task(&task_id).cloned();
                    let _ = reply.send(result);
                }
                ActorCommand::Stop(task_id, reply) => {
                    let result = self.stop(&task_id).await;
                    let _ = reply.send(result);
                }
                ActorCommand::Remove(task_id, reply) => {
                    let result = self.remove(&task_id).await;
                    let _ = reply.send(result);
                }
                ActorCommand::Shutdown(reply) => {
                    let result = self.shutdown().await;
                    let _ = reply.send(result);
                }
                ActorCommand::Complete(completion) => {
                    if let Err(error) = self.complete(completion).await {
                        eprintln!("[agent-bridge] failed to complete task: {error}");
                    }
                }
            }
        }
    }

    async fn spawn(&mut self, arguments: Value) -> Result<Value, String> {
        let input = validate_spawn_arguments(arguments)?;
        let task_id = self.next_task_id();
        let created_at = now_iso();
        let task_dir = self.state_dir.join("tasks").join(&task_id);
        fs::create_dir_all(&task_dir)
            .await
            .map_err(|error| error.to_string())?;
        let cwd = safe_cwd(input.cwd.as_deref())?;
        let original_cwd = cwd.clone();
        let mut run_cwd = cwd.clone();
        let mut worktree_path = None;
        let mut worktree_managed = false;
        if input.isolation == Some(Isolation::Worktree) {
            let worktree = create_worktree(
                &self.state_dir,
                &cwd,
                input.provider,
                input.mode,
                &task_id,
                input.worktree_name.as_deref(),
            )
            .await?;
            run_cwd = worktree.clone();
            worktree_path = Some(worktree);
            worktree_managed = true;
        }
        let timeout_seconds = TimeoutSeconds::new(input.timeout_seconds).get();
        let provider_task = ProviderTask {
            provider: input.provider,
            mode: input.mode,
            prompt: &input.prompt,
            title: input.title.as_deref(),
            cwd: &run_cwd,
            timeout_seconds,
            model: input.model.as_deref(),
            effort: input.effort.as_deref(),
            thinking: input.thinking.as_deref(),
        };
        let command = provider::build_command(&provider_task)?;
        let mut record = TaskRecord {
            task_id: task_id.clone(),
            provider: input.provider,
            mode: input.mode,
            title: input.title,
            status: TaskStatus::Queued,
            cwd: run_cwd,
            original_cwd: Some(original_cwd),
            isolation: input.isolation.unwrap_or(Isolation::None),
            worktree_managed,
            worktree_path,
            task_dir: task_dir.display().to_string(),
            command: command.command.clone(),
            args: command.args.clone(),
            timeout_seconds,
            pid: None,
            created_at: created_at.clone(),
            updated_at: created_at,
            started_at: None,
            completed_at: None,
            exit_code: None,
            signal: None,
            error: None,
            error_type: None,
            diagnostic: None,
            git_status: None,
            git_diff: None,
            changed_files: None,
        };

        match launch_task(task_id.clone(), command, task_dir, self.tx.clone()).await {
            Ok(active) => {
                record.pid = active.pid;
                transition_status(&mut record, TaskStatus::Running)?;
                record.started_at = Some(now_iso());
                record.updated_at = record.started_at.clone().unwrap();
                self.active.insert(task_id.clone(), active);
            }
            Err(error) => {
                transition_status(&mut record, TaskStatus::Failed)?;
                record.error = Some(error);
                record.error_type = Some(ErrorType::ProviderStartError);
                record.completed_at = Some(now_iso());
                record.updated_at = record.completed_at.clone().unwrap();
            }
        }
        self.registry.tasks.insert(task_id.clone(), record);
        self.save().await?;
        Ok(public_task(self.registry.tasks.get(&task_id).unwrap()))
    }

    async fn stop(&mut self, task_id: &str) -> Result<Value, String> {
        let active = self.active.remove(task_id);
        let task = self.require_task_mut(task_id)?;
        if active.is_none() {
            if is_final(task.status) {
                return Ok(public_task(task));
            }
            return Err(format!("task is not running: {task_id}"));
        }
        transition_status(task, TaskStatus::Stopped)?;
        task.error_type = Some(ErrorType::Stopped);
        task.updated_at = now_iso();
        let public = public_task(task);
        self.save().await?;
        if let Some(mut active) = active {
            if let Some(pid) = active.pid {
                send_signal(pid, libc::SIGTERM);
            }
            if let Some(cancel) = active.cancel.take() {
                let _ = cancel.send(());
            }
        }
        Ok(public)
    }

    async fn remove(&mut self, task_id: &str) -> Result<Value, String> {
        let task = self.require_task_mut(task_id)?;
        if matches!(task.status, TaskStatus::Running | TaskStatus::Queued) {
            return Err("cannot remove a running task; stop it first".to_string());
        }
        if task.worktree_managed && task.worktree_path.is_some() {
            let worktree_path = task.worktree_path.clone().unwrap();
            let cleanup_cwd = task
                .original_cwd
                .clone()
                .unwrap_or_else(|| task.cwd.clone());
            run_git(&["worktree", "remove", "-f", &worktree_path], &cleanup_cwd).await?;
        }
        let task_dir = task.task_dir.clone();
        transition_status(task, TaskStatus::Removed)?;
        task.updated_at = now_iso();
        self.save().await?;
        let _ = fs::remove_dir_all(task_dir).await;
        Ok(json!({ "taskId": task_id, "status": "removed" }))
    }

    async fn shutdown(&mut self) -> Result<(), String> {
        let pids: Vec<u32> = self
            .active
            .values()
            .filter_map(|active| active.pid)
            .collect();
        for pid in &pids {
            send_signal(*pid, libc::SIGTERM);
        }
        sleep(CHILD_SHUTDOWN_GRACE).await;
        for active in self.active.values_mut() {
            if let Some(pid) = active.pid {
                send_signal(pid, libc::SIGKILL);
            }
            if let Some(cancel) = active.cancel.take() {
                let _ = cancel.send(());
            }
        }
        Ok(())
    }

    async fn complete(&mut self, completion: TaskCompletion) -> Result<(), String> {
        self.active.remove(&completion.task_id);
        let task = self.require_task_mut(&completion.task_id)?;
        if task.status != TaskStatus::Stopped {
            transition_status(task, completion.status)?;
            task.error = completion.error;
            task.error_type = completion.error_type;
            task.diagnostic = completion.diagnostic;
        } else if task.error.is_none() {
            task.error = Some(format!(
                "task stopped with signal {}",
                completion
                    .signal
                    .clone()
                    .unwrap_or_else(|| "SIGTERM".to_string())
            ));
        }
        task.exit_code = completion.exit_code;
        task.signal = completion.signal;
        task.completed_at = Some(now_iso());
        task.updated_at = task.completed_at.clone().unwrap();
        let snapshot = git_snapshot(&task.cwd).await;
        task.git_status = Some(snapshot.git_status);
        task.git_diff = Some(snapshot.git_diff);
        task.changed_files = Some(snapshot.changed_files);
        let result_path = PathBuf::from(&task.task_dir).join("result.json");
        let result_json = serde_json::to_vec_pretty(task).map_err(|error| error.to_string())?;
        fs::write(result_path, result_json)
            .await
            .map_err(|error| error.to_string())?;
        self.save().await
    }

    fn require_task(&self, task_id: &str) -> Result<&TaskRecord, String> {
        self.registry
            .tasks
            .get(task_id)
            .filter(|task| task.status != TaskStatus::Removed)
            .ok_or_else(|| format!("Unknown task: {task_id}"))
    }

    fn require_task_mut(&mut self, task_id: &str) -> Result<&mut TaskRecord, String> {
        self.registry
            .tasks
            .get_mut(task_id)
            .filter(|task| task.status != TaskStatus::Removed)
            .ok_or_else(|| format!("Unknown task: {task_id}"))
    }

    fn next_task_id(&self) -> String {
        loop {
            let task_id = format!("task_{}", Uuid::new_v4().simple());
            if !self.registry.tasks.contains_key(&task_id) {
                return task_id;
            }
        }
    }

    async fn save(&self) -> Result<(), String> {
        save_registry(&self.state_dir, &self.registry).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Registry {
    #[serde(default)]
    tasks: BTreeMap<String, TaskRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
    pub task_id: String,
    pub provider: ProviderKind,
    pub mode: crate::domain::TaskMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub status: TaskStatus,
    pub cwd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_cwd: Option<String>,
    pub isolation: Isolation,
    #[serde(default)]
    pub worktree_managed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree_path: Option<String>,
    pub task_dir: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub timeout_seconds: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_type: Option<ErrorType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_diff: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed_files: Option<Vec<String>>,
}

struct ActiveTask {
    pid: Option<u32>,
    cancel: Option<oneshot::Sender<()>>,
}

struct TaskCompletion {
    task_id: String,
    status: TaskStatus,
    exit_code: Option<i32>,
    signal: Option<String>,
    error: Option<String>,
    error_type: Option<ErrorType>,
    diagnostic: Option<Value>,
}

struct ChildIoDrains {
    stdout: Option<tokio::task::JoinHandle<()>>,
    stderr: Option<tokio::task::JoinHandle<()>>,
}

async fn launch_task(
    task_id: String,
    command: ProviderCommand,
    task_dir: PathBuf,
    tx: mpsc::Sender<ActorCommand>,
) -> Result<ActiveTask, String> {
    if command.provider == ProviderKind::Claude
        && let (Some(socket_path), Some(claude_command)) = (
            crate::claude_host::socket_path_from_env(),
            command.claude_host.clone(),
        )
    {
        let (cancel_tx, cancel_rx) = oneshot::channel();
        tokio::spawn(async move {
            let completion = run_host_task(
                task_id,
                command,
                claude_command,
                socket_path,
                task_dir,
                cancel_rx,
            )
            .await;
            if tx.send(ActorCommand::Complete(completion)).await.is_err() {
                eprintln!("[agent-bridge] task manager dropped completion message");
                std::process::abort();
            }
        });
        return Ok(ActiveTask {
            pid: None,
            cancel: Some(cancel_tx),
        });
    }
    let stdout_path = task_dir.join("stdout.log");
    let stderr_path = task_dir.join("stderr.log");
    let mut child = ProcessCommand::new(&command.command)
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
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
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
    let drains = ChildIoDrains {
        stdout: child
            .stdout
            .take()
            .map(|stdout| tokio::spawn(drain_log(stdout, stdout_path))),
        stderr: child
            .stderr
            .take()
            .map(|stderr| tokio::spawn(drain_log(stderr, stderr_path))),
    };
    tokio::spawn(async move {
        let completion = wait_for_child(
            task_id,
            pid,
            command.timeout_seconds,
            child,
            command,
            task_dir,
            drains,
        )
        .await;
        if tx.send(ActorCommand::Complete(completion)).await.is_err() {
            eprintln!("[agent-bridge] task manager dropped completion message");
            std::process::abort();
        }
    });
    Ok(ActiveTask {
        pid: Some(pid),
        cancel: None,
    })
}

async fn run_host_task(
    task_id: String,
    command: ProviderCommand,
    claude_command: crate::claude_host::ClaudeHostCommand,
    socket_path: PathBuf,
    task_dir: PathBuf,
    cancel_rx: oneshot::Receiver<()>,
) -> TaskCompletion {
    let result = tokio::select! {
        result = crate::claude_host::run_claude(&socket_path, &claude_command) => result,
        _ = cancel_rx => {
            return TaskCompletion {
                task_id,
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
            complete_host_response(task_id, command, task_dir, response).await
        }
        Ok(response) => TaskCompletion {
            task_id,
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
            task_id,
            status: TaskStatus::Failed,
            exit_code: None,
            signal: None,
            error: Some(error),
            error_type: Some(ErrorType::ProviderStartError),
            diagnostic: None,
        },
    }
}

async fn complete_host_response(
    task_id: String,
    command: ProviderCommand,
    task_dir: PathBuf,
    response: crate::claude_host::HostResponse,
) -> TaskCompletion {
    let Some(crate::claude_host::HostResult::Run {
        exit_code,
        signal,
        stdout,
        stderr,
        failure_category,
        ..
    }) = response.result
    else {
        return TaskCompletion {
            task_id,
            status: TaskStatus::Failed,
            exit_code: None,
            signal: None,
            error: Some("host runner returned unexpected response".to_string()),
            error_type: Some(ErrorType::ProviderOutputError),
            diagnostic: None,
        };
    };
    let stdout_bytes = stdout.as_bytes().to_vec();
    let stderr_bytes = stderr.as_bytes().to_vec();
    let _ = fs::write(task_dir.join("stdout.log"), &stdout_bytes).await;
    let _ = fs::write(task_dir.join("stderr.log"), &stderr_bytes).await;
    let success = exit_code == Some(0) && failure_category.is_none();
    if success && !claude_output_is_parseable(&stdout_bytes) {
        return TaskCompletion {
            task_id,
            status: TaskStatus::Failed,
            exit_code,
            signal: signal.clone(),
            error: Some("claude provider output was not parseable".to_string()),
            error_type: Some(ErrorType::ProviderOutputError),
            diagnostic: Some(task_diagnostic(
                &command,
                "provider_output_error",
                command.timeout_seconds * 1000,
                exit_code,
                signal_name_from_string(signal.as_deref()),
                &stdout_bytes,
                &stderr_bytes,
            )),
        };
    }
    if success {
        TaskCompletion {
            task_id,
            status: TaskStatus::Succeeded,
            exit_code,
            signal,
            error: None,
            error_type: None,
            diagnostic: None,
        }
    } else {
        let category = failure_category.unwrap_or_else(|| "provider_exit_error".to_string());
        TaskCompletion {
            task_id,
            status: TaskStatus::Failed,
            exit_code,
            signal: signal.clone(),
            error: Some(category.clone()),
            error_type: Some(if category == "provider_timeout" {
                ErrorType::Timeout
            } else {
                ErrorType::ProviderExitError
            }),
            diagnostic: Some(task_diagnostic(
                &command,
                &category,
                command.timeout_seconds * 1000,
                exit_code,
                signal,
                &stdout_bytes,
                &stderr_bytes,
            )),
        }
    }
}

fn signal_name_from_string(signal: Option<&str>) -> Option<String> {
    signal.map(str::to_string)
}

async fn wait_for_child(
    task_id: String,
    pid: u32,
    timeout_seconds: i64,
    mut child: tokio::process::Child,
    command: ProviderCommand,
    task_dir: PathBuf,
    drains: ChildIoDrains,
) -> TaskCompletion {
    let wait = timeout(Duration::from_secs(timeout_seconds as u64), child.wait()).await;
    let mut timed_out = false;
    let output: Result<std::process::ExitStatus, String> = match wait {
        Ok(wait_result) => wait_result.map_err(|error| error.to_string()),
        Err(_) => {
            timed_out = true;
            send_signal(pid, libc::SIGTERM);
            match timeout(CHILD_SHUTDOWN_GRACE, child.wait()).await {
                Ok(result) => result,
                Err(_) => {
                    send_signal(pid, libc::SIGKILL);
                    child.wait().await
                }
            }
            .map(|status| (status, true))
            .map_err(|error| error.to_string())
            .map(|(status, _)| status)
        }
    };
    if let Some(handle) = drains.stdout {
        let _ = timeout(CHILD_SHUTDOWN_GRACE, handle).await;
    }
    if let Some(handle) = drains.stderr {
        let _ = timeout(CHILD_SHUTDOWN_GRACE, handle).await;
    }
    match output {
        Ok(status) if status.success() => {
            if command_provider_hint(&command) == ProviderKind::Claude {
                let stdout = std::fs::read(task_dir.join("stdout.log")).unwrap_or_default();
                let stderr = std::fs::read(task_dir.join("stderr.log")).unwrap_or_default();
                if !claude_output_is_parseable(&stdout) {
                    return TaskCompletion {
                        task_id,
                        status: TaskStatus::Failed,
                        exit_code: status.code(),
                        signal: signal_name(&status),
                        error: Some("claude provider output was not parseable".to_string()),
                        error_type: Some(ErrorType::ProviderOutputError),
                        diagnostic: Some(task_diagnostic(
                            &command,
                            "provider_output_error",
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
                task_id,
                status: TaskStatus::Succeeded,
                exit_code: status.code(),
                signal: signal_name(&status),
                error: None,
                error_type: None,
                diagnostic: None,
            }
        }
        Ok(status) => {
            let signal = signal_name(&status);
            let stdout = std::fs::read(task_dir.join("stdout.log")).unwrap_or_default();
            let stderr = std::fs::read(task_dir.join("stderr.log")).unwrap_or_default();
            TaskCompletion {
                task_id,
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
                diagnostic: Some(task_diagnostic(
                    &command,
                    if timed_out {
                        "provider_timeout"
                    } else {
                        "provider_exit_error"
                    },
                    timeout_seconds * 1000,
                    status.code(),
                    signal,
                    &stdout,
                    &stderr,
                )),
            }
        }
        Err(error) => TaskCompletion {
            task_id,
            status: TaskStatus::Failed,
            exit_code: None,
            signal: None,
            error: Some(error),
            error_type: Some(ErrorType::ProviderExitError),
            diagnostic: None,
        },
    }
}

fn claude_output_is_parseable(stdout: &[u8]) -> bool {
    let text = String::from_utf8_lossy(stdout);
    text.lines().any(|line| {
        let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
            return false;
        };
        value
            .get("result")
            .and_then(Value::as_str)
            .is_some_and(|result| !result.is_empty())
    })
}

fn task_diagnostic(
    command: &ProviderCommand,
    failure_category: &str,
    timeout_ms: i64,
    exit_code: Option<i32>,
    signal: Option<String>,
    stdout: &[u8],
    stderr: &[u8],
) -> Value {
    let redactions = diagnostic_redactions(command);
    json!({
        "failureCategory": failure_category,
        "provider": command_provider_hint(command).as_str(),
        "commandKind": claude_command_kind(command),
        "commandPath": claude_command_path(command),
        "launchStrategy": if command.provider == ProviderKind::Claude && crate::claude_host::socket_path_from_env().is_some() { "host_runner" } else { "direct" },
        "startupVerified": false,
        "timeoutMs": timeout_ms,
        "exitCode": exit_code,
        "signal": signal,
        "stdoutExcerpt": diagnostic_excerpt(stdout, &redactions),
        "stderrExcerpt": diagnostic_excerpt(stderr, &redactions)
    })
}

fn diagnostic_redactions(command: &ProviderCommand) -> Vec<String> {
    let mut redactions = command.redactions.clone();
    redactions.extend(
        provider::provider_env(command.provider)
            .into_iter()
            .filter(|(key, _)| {
                key.contains("KEY") || key.contains("TOKEN") || key.contains("SECRET")
            })
            .map(|(_, value)| value)
            .filter(|value| !value.is_empty()),
    );
    redactions
}

fn claude_command_kind(command: &ProviderCommand) -> String {
    command
        .command_kind
        .as_deref()
        .unwrap_or("native-claude")
        .to_string()
}

fn claude_command_path(command: &ProviderCommand) -> String {
    if command.command == "/bin/zsh" {
        return command
            .args
            .get(3)
            .cloned()
            .unwrap_or_else(|| command.command.clone());
    }
    command.command.clone()
}

fn diagnostic_excerpt(bytes: &[u8], redactions: &[String]) -> String {
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

async fn drain_log(mut reader: impl tokio::io::AsyncRead + Unpin, path: PathBuf) {
    let mut file_bytes = 0usize;
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
        file_bytes += take;
    }
}

struct GitSnapshot {
    git_status: String,
    git_diff: String,
    changed_files: Vec<String>,
}

async fn create_worktree(
    state_dir: &Path,
    cwd: &str,
    provider: ProviderKind,
    mode: crate::domain::TaskMode,
    task_id: &str,
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
            &task_id[task_id.len() - 8..]
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

async fn git_snapshot(cwd: &str) -> GitSnapshot {
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

async fn run_git_stdout(args: &[&str], cwd: &str) -> Result<String, String> {
    let output = git_command(args, cwd).await?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

async fn run_git(args: &[&str], cwd: &str) -> Result<(), String> {
    let _ = git_command(args, cwd).await?;
    Ok(())
}

async fn git_command(args: &[&str], cwd: &str) -> Result<std::process::Output, String> {
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

fn cap_string(value: String, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        value
    } else {
        String::from_utf8_lossy(&value.as_bytes()[..max_bytes]).to_string()
    }
}

fn validate_spawn_arguments(arguments: Value) -> Result<TaskPreviewInput, String> {
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

fn public_task(task: &TaskRecord) -> Value {
    let is_final = is_final(task.status);
    let phase = match task.status {
        TaskStatus::Queued => TaskPhase::Pending,
        TaskStatus::Running => TaskPhase::Active,
        _ => TaskPhase::Done,
    };
    json!({
        "taskId": task.task_id,
        "provider": task.provider,
        "mode": task.mode,
        "title": task.title,
        "status": task.status,
        "cwd": task.cwd,
        "isolation": task.isolation,
        "worktreePath": task.worktree_path,
        "pid": task.pid,
        "createdAt": task.created_at,
        "updatedAt": task.updated_at,
        "startedAt": task.started_at,
        "completedAt": task.completed_at,
        "isFinal": is_final,
        "phase": phase,
        "durationMs": duration_ms(task),
        "errorType": task.error_type
    })
}

fn review_packet(task: &TaskRecord, stdout_truncated: bool, stderr_truncated: bool) -> Value {
    let is_final = is_final(task.status);
    let git_status = task.git_status.clone().unwrap_or_default();
    let changed_files = task.changed_files.clone().unwrap_or_default();
    let has_changes = !changed_files.is_empty() || !git_status.trim().is_empty();
    json!({
        "taskId": task.task_id,
        "provider": task.provider,
        "mode": task.mode,
        "title": task.title,
        "status": task.status,
        "cwd": task.cwd,
        "isolation": task.isolation,
        "worktreePath": task.worktree_path,
        "isFinal": is_final,
        "phase": match task.status {
            TaskStatus::Queued => TaskPhase::Pending,
            TaskStatus::Running => TaskPhase::Active,
            _ => TaskPhase::Done,
        },
        "hasChanges": has_changes,
        "gitStatusSummary": git_status,
        "changedFiles": changed_files,
        "exitCode": task.exit_code,
        "signal": task.signal,
        "errorType": task.error_type,
        "diagnostic": task.diagnostic,
        "stdoutTruncated": stdout_truncated,
        "stderrTruncated": stderr_truncated,
        "recommendedActions": recommended_actions(task, has_changes)
    })
}

fn recommended_actions(task: &TaskRecord, has_changes: bool) -> Vec<&'static str> {
    if !is_final(task.status) {
        return vec![
            "Use task_wait with a bounded timeout.",
            "Use task_logs with line cursors to inspect incremental output.",
            "Use task_status to confirm whether the task is still active.",
            "Use task_stop if the task is no longer useful.",
        ];
    }

    let mut actions =
        vec!["Inspect stdout, stderr, diagnostics, git status, diff, and changed files."];
    if has_changes {
        actions.push("Inspect gitStatus, gitDiff, and changedFiles before verification.");
    }
    if task.error_type.is_some()
        || matches!(task.status, TaskStatus::Failed | TaskStatus::FailedStale)
    {
        actions.push("Inspect logs and diagnostic metadata before deciding whether to rerun.");
        actions
            .push("Decide whether to rerun with a narrower prompt, continue manually, or discard.");
    } else {
        actions.push("Run the relevant project verification before claiming completion.");
    }
    if task.worktree_managed {
        actions.push("Call task_remove only after inspecting the managed worktree result.");
    }
    actions
}

fn is_final(status: TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Succeeded | TaskStatus::Failed | TaskStatus::Stopped | TaskStatus::FailedStale
    )
}

fn transition_status(task: &mut TaskRecord, next: TaskStatus) -> Result<(), String> {
    let allowed = matches!(
        (task.status, next),
        (TaskStatus::Queued, TaskStatus::Running)
            | (TaskStatus::Queued, TaskStatus::Failed)
            | (TaskStatus::Queued, TaskStatus::FailedStale)
            | (TaskStatus::Running, TaskStatus::Succeeded)
            | (TaskStatus::Running, TaskStatus::Failed)
            | (TaskStatus::Running, TaskStatus::Stopped)
            | (TaskStatus::Running, TaskStatus::FailedStale)
            | (TaskStatus::Succeeded, TaskStatus::Removed)
            | (TaskStatus::Failed, TaskStatus::Removed)
            | (TaskStatus::Stopped, TaskStatus::Removed)
            | (TaskStatus::FailedStale, TaskStatus::Removed)
    );
    if allowed || task.status == next {
        task.status = next;
        Ok(())
    } else {
        Err(format!(
            "invalid task state transition: {:?} -> {:?}",
            task.status, next
        ))
    }
}

fn duration_ms(task: &TaskRecord) -> Value {
    let Some(started_at) = task.started_at.as_deref() else {
        return Value::Null;
    };
    let end = task
        .completed_at
        .as_deref()
        .unwrap_or(task.updated_at.as_str());
    let Ok(start) = chrono::DateTime::parse_from_rfc3339(started_at) else {
        return Value::Null;
    };
    let Ok(end) = chrono::DateTime::parse_from_rfc3339(end) else {
        return Value::Null;
    };
    json!((end - start).num_milliseconds())
}

async fn load_registry(state_dir: &Path) -> Result<Registry, String> {
    cleanup_registry_temps(state_dir).await?;
    let path = state_dir.join("registry.json");
    match fs::read_to_string(&path).await {
        Ok(text) => serde_json::from_str(&text)
            .map_err(|error| format!("failed to parse registry.json: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Registry {
            tasks: BTreeMap::new(),
        }),
        Err(error) => Err(error.to_string()),
    }
}

async fn cleanup_registry_temps(state_dir: &Path) -> Result<(), String> {
    let mut entries = match fs::read_dir(state_dir).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|error| error.to_string())?
    {
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with("registry.json.tmp-")
        {
            let _ = fs::remove_file(entry.path()).await;
        }
    }
    Ok(())
}

async fn save_registry(state_dir: &Path, registry: &Registry) -> Result<(), String> {
    fs::create_dir_all(state_dir)
        .await
        .map_err(|error| error.to_string())?;
    let registry_path = state_dir.join("registry.json");
    let tmp_path = state_dir.join(format!(
        "registry.json.tmp-{}-{}",
        std::process::id(),
        Uuid::new_v4().simple()
    ));
    let bytes = serde_json::to_vec_pretty(registry).map_err(|error| error.to_string())?;
    fs::write(&tmp_path, bytes)
        .await
        .map_err(|error| error.to_string())?;
    fs::rename(&tmp_path, &registry_path)
        .await
        .map_err(|error| error.to_string())
}

struct CappedText {
    text: String,
    truncated: bool,
}

async fn read_capped_file(path: &Path, max_bytes: usize) -> Result<CappedText, String> {
    match fs::read(path).await {
        Ok(bytes) => {
            let truncated = bytes.len() > max_bytes;
            let capped = &bytes[..bytes.len().min(max_bytes)];
            Ok(CappedText {
                text: String::from_utf8_lossy(capped).to_string(),
                truncated,
            })
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(CappedText {
            text: String::new(),
            truncated: false,
        }),
        Err(error) => Err(error.to_string()),
    }
}

struct SlicedLines {
    text: String,
    next_line: usize,
}

fn slice_lines(text: &str, start_line: usize) -> SlicedLines {
    if text.is_empty() {
        return SlicedLines {
            text: String::new(),
            next_line: 0,
        };
    }
    let ends_with_newline = text.ends_with('\n');
    let mut lines: Vec<&str> = text.lines().collect();
    let total_lines = lines.len();
    if ends_with_newline {
        lines.push("");
    }
    let mut sliced = lines
        .into_iter()
        .skip(start_line)
        .collect::<Vec<_>>()
        .join("\n");
    if ends_with_newline && !sliced.is_empty() {
        sliced.push('\n');
    }
    SlicedLines {
        text: sliced,
        next_line: total_lines,
    }
}

fn safe_cwd(cwd: Option<&str>) -> Result<String, String> {
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

fn configured_workspace_roots() -> Result<Vec<PathBuf>, String> {
    let roots: Vec<PathBuf> = env::var_os("AGENT_BRIDGE_WORKSPACES")
        .map(|value| {
            env::split_paths(&value)
                .filter(|path| !path.as_os_str().is_empty())
                .collect()
        })
        .unwrap_or_else(|| vec![env::current_dir().unwrap_or_else(|_| PathBuf::from("."))]);
    let roots = if roots.is_empty() {
        vec![env::current_dir().map_err(|error| error.to_string())?]
    } else {
        roots
    };
    roots
        .into_iter()
        .map(|root| root.canonicalize().map_err(|error| error.to_string()))
        .collect()
}

fn normalize_wait_ms(value: Option<i64>) -> i64 {
    value.unwrap_or(30_000).clamp(0, MAX_WAIT_MS)
}

fn normalize_max_bytes(value: Option<i64>) -> usize {
    value
        .unwrap_or(MAX_LOG_BYTES as i64)
        .clamp(1, MAX_LOG_BYTES as i64) as usize
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn expand_home(value: &str) -> PathBuf {
    if value == "~" {
        return env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(value));
    }
    if let Some(rest) = value.strip_prefix("~/") {
        return env::var("HOME")
            .map(|home| PathBuf::from(home).join(rest))
            .unwrap_or_else(|_| PathBuf::from(value));
    }
    PathBuf::from(value)
}

fn send_signal(pid: u32, signal: i32) {
    unsafe {
        libc::kill(pid as libc::pid_t, signal);
    }
}

#[cfg(unix)]
fn signal_name(status: &std::process::ExitStatus) -> Option<String> {
    use std::os::unix::process::ExitStatusExt;
    status.signal().map(|signal| match signal {
        libc::SIGTERM => "SIGTERM".to_string(),
        libc::SIGKILL => "SIGKILL".to_string(),
        other => format!("SIG{other}"),
    })
}

#[cfg(not(unix))]
fn signal_name(_status: &std::process::ExitStatus) -> Option<String> {
    None
}

fn command_provider_hint(command: &ProviderCommand) -> ProviderKind {
    command.provider
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Stdio;

    fn temp_dir(name: &str) -> PathBuf {
        let path = env::temp_dir().join(format!(
            "agent-bridge-mcp-{name}-{}",
            Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn sample_task(status: TaskStatus) -> TaskRecord {
        TaskRecord {
            task_id: "task_11111111111111111111111111111111".to_string(),
            provider: ProviderKind::Codex,
            mode: crate::domain::TaskMode::Review,
            title: None,
            status,
            cwd: ".".to_string(),
            original_cwd: None,
            isolation: Isolation::None,
            worktree_managed: false,
            worktree_path: None,
            task_dir: ".".to_string(),
            command: String::new(),
            args: Vec::new(),
            timeout_seconds: 1,
            pid: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            started_at: None,
            completed_at: None,
            exit_code: None,
            signal: None,
            error: None,
            error_type: None,
            diagnostic: None,
            git_status: None,
            git_diff: None,
            changed_files: None,
        }
    }

    #[test]
    fn transition_status_rejects_illegal_moves() {
        let mut task = sample_task(TaskStatus::Queued);
        transition_status(&mut task, TaskStatus::Running).unwrap();
        transition_status(&mut task, TaskStatus::Succeeded).unwrap();

        assert!(transition_status(&mut task, TaskStatus::Running).is_err());
    }

    #[tokio::test]
    async fn load_registry_removes_known_temp_files() {
        let dir = temp_dir("registry-temp");
        let tmp = dir.join("registry.json.tmp-test");
        fs::write(&tmp, b"partial").await.unwrap();

        let registry = load_registry(&dir).await.unwrap();

        assert!(registry.tasks.is_empty());
        assert!(!tmp.exists());
    }

    #[tokio::test]
    async fn load_registry_rejects_corrupted_canonical_file() {
        let dir = temp_dir("registry-corrupt");
        fs::write(dir.join("registry.json"), b"{not-json")
            .await
            .unwrap();

        let error = load_registry(&dir).await.unwrap_err();

        assert!(error.contains("failed to parse registry.json"));
    }

    #[tokio::test]
    async fn send_signal_sends_sigterm_to_unix_process() {
        let mut child = ProcessCommand::new("/bin/sleep")
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let pid = child.id().unwrap();

        send_signal(pid, libc::SIGTERM);
        let status = timeout(Duration::from_secs(3), child.wait())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(signal_name(&status).as_deref(), Some("SIGTERM"));
    }
}
