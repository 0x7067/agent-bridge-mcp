use crate::domain::{
    ErrorType, Isolation, LaunchProfile, ProviderKind, TaskPhase, TaskStatus, TimeoutSeconds,
    WorktreeName,
};
use crate::provider::{self, ProviderCommand, ProviderTask};
use crate::tools::TaskPreviewInput;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::env;
use std::io::{Read, Seek, SeekFrom};
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
const MAX_OBSERVE_MS: i64 = 120_000;
const MAX_OBSERVE_EVENTS: usize = 500;
const PROGRESS_TRANSCRIPT_TAIL_BYTES: u64 = 64 * 1024;
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

    pub async fn list(&self, arguments: Value) -> Result<Value, String> {
        self.request(|reply| ActorCommand::List(arguments, reply))
            .await
    }

    pub async fn status(&self, agent_id: String) -> Result<Value, String> {
        self.request(|reply| ActorCommand::Status(agent_id, reply))
            .await
    }

    pub async fn wait(&self, agent_id: String, timeout_ms: Option<i64>) -> Result<Value, String> {
        let deadline = Instant::now() + Duration::from_millis(normalize_wait_ms(timeout_ms) as u64);
        loop {
            let status = self.status(agent_id.clone()).await?;
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

    pub async fn observe(
        &self,
        agent_id: String,
        cursor: Option<u64>,
        limit: Option<u64>,
        timeout_ms: Option<i64>,
    ) -> Result<Value, String> {
        let cursor = cursor.unwrap_or(0) as usize;
        let limit = normalize_observe_limit(limit);
        let timeout_ms = normalize_observe_ms(timeout_ms);
        let deadline = Instant::now() + Duration::from_millis(timeout_ms as u64);
        loop {
            let task: TaskRecord = self
                .request(|reply| ActorCommand::Get(agent_id.clone(), reply))
                .await?;
            let transcript = read_transcript(&task, cursor, limit).await?;
            let next_cursor = transcript["nextCursor"].as_u64().unwrap_or(cursor as u64);
            let has_events = transcript["events"]
                .as_array()
                .is_some_and(|events| !events.is_empty());
            if has_events || is_final(task.status) {
                return Ok(observe_payload(task, transcript, false));
            }
            if Instant::now() >= deadline {
                return Ok(observe_payload(task, transcript, true));
            }
            if next_cursor > cursor as u64 {
                return Ok(observe_payload(task, transcript, false));
            }
            sleep(Duration::from_millis(50)).await;
        }
    }

    pub async fn logs(
        &self,
        agent_id: String,
        max_bytes: Option<i64>,
        stdout_line: Option<usize>,
        stderr_line: Option<usize>,
    ) -> Result<Value, String> {
        let task: TaskRecord = self
            .request(|reply| ActorCommand::Get(agent_id.clone(), reply))
            .await?;
        let stdout = read_capped_file(
            &PathBuf::from(&task.agent_dir).join("stdout.log"),
            normalize_max_bytes(max_bytes),
        )
        .await?;
        let stderr = read_capped_file(
            &PathBuf::from(&task.agent_dir).join("stderr.log"),
            normalize_max_bytes(max_bytes),
        )
        .await?;
        let stdout_lines = slice_lines(&stdout.text, stdout_line.unwrap_or(0));
        let stderr_lines = slice_lines(&stderr.text, stderr_line.unwrap_or(0));
        Ok(json!({
            "agentId": agent_id,
            "status": task.status,
            "stdout": stdout_lines.text,
            "stderr": stderr_lines.text,
            "stdoutTruncated": stdout.truncated,
            "stderrTruncated": stderr.truncated,
            "nextStdoutLine": stdout_lines.next_line,
            "nextStderrLine": stderr_lines.next_line
        }))
    }

    pub async fn transcript(
        &self,
        agent_id: String,
        cursor: Option<u64>,
        limit: Option<u64>,
    ) -> Result<Value, String> {
        let task: TaskRecord = self
            .request(|reply| ActorCommand::Get(agent_id.clone(), reply))
            .await?;
        read_transcript(
            &task,
            cursor.unwrap_or(0) as usize,
            limit.unwrap_or(200) as usize,
        )
        .await
    }

    pub async fn result(&self, agent_id: String, max_bytes: Option<i64>) -> Result<Value, String> {
        let task: TaskRecord = self
            .request(|reply| ActorCommand::InspectResult(agent_id.clone(), reply))
            .await?;
        let logs = self.logs(agent_id, max_bytes, None, None).await?;
        let mut public = public_task(&task);
        public["exitCode"] = task.exit_code.map_or(Value::Null, Value::from);
        public["signal"] = task.signal.clone().map_or(Value::Null, Value::from);
        public["error"] = task.error.clone().map_or(Value::Null, Value::from);
        public["stdout"] = logs["stdout"].clone();
        public["stderr"] = logs["stderr"].clone();
        public["stdoutTruncated"] = logs["stdoutTruncated"].clone();
        public["stderrTruncated"] = logs["stderrTruncated"].clone();
        public["diagnostic"] = task.diagnostic.clone().unwrap_or(Value::Null);
        public["profile"] = json!(task.profile);
        public["promptStrategy"] = Value::String(task.prompt_strategy.clone());
        public["profileDiagnostics"] = task.profile_diagnostics.clone().unwrap_or(Value::Null);
        public["transcriptAvailable"] = Value::Bool(task.transcript_available);
        public["finalResultDetected"] = Value::Bool(task.final_result_detected);
        public["partialResultDetected"] = Value::Bool(task.partial_result_detected);
        public["transcriptDiagnostic"] = task.transcript_diagnostic.clone().unwrap_or(Value::Null);
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

    pub async fn stop(&self, agent_id: String) -> Result<Value, String> {
        self.request(|reply| ActorCommand::Stop(agent_id, reply))
            .await
    }

    pub async fn remove(&self, agent_id: String) -> Result<Value, String> {
        self.request(|reply| ActorCommand::Remove(agent_id, reply))
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
    List(Value, oneshot::Sender<Result<Value, String>>),
    Status(String, oneshot::Sender<Result<Value, String>>),
    Get(String, oneshot::Sender<Result<TaskRecord, String>>),
    InspectResult(String, oneshot::Sender<Result<TaskRecord, String>>),
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
                ActorCommand::List(arguments, reply) => {
                    let result = list_tasks(&self.registry, arguments);
                    let _ = reply.send(result);
                }
                ActorCommand::Status(agent_id, reply) => {
                    let result = self.require_task(&agent_id).map(public_task);
                    let _ = reply.send(result);
                }
                ActorCommand::Get(agent_id, reply) => {
                    let result = self.require_task(&agent_id).cloned();
                    let _ = reply.send(result);
                }
                ActorCommand::InspectResult(agent_id, reply) => {
                    let result = self.inspect_result(&agent_id).await;
                    let _ = reply.send(result);
                }
                ActorCommand::Stop(agent_id, reply) => {
                    let result = self.stop(&agent_id).await;
                    let _ = reply.send(result);
                }
                ActorCommand::Remove(agent_id, reply) => {
                    let result = self.remove(&agent_id).await;
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
        let agent_id = self.next_agent_id();
        let created_at = now_iso();
        let agent_dir = self.state_dir.join("tasks").join(&agent_id);
        fs::create_dir_all(&agent_dir)
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
                &agent_id,
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
            profile: input.profile.unwrap_or(LaunchProfile::Bridge),
        };
        let command = provider::build_command(&provider_task)?;
        let mut record = TaskRecord {
            agent_id: agent_id.clone(),
            provider: input.provider,
            mode: input.mode,
            title: input.title,
            status: TaskStatus::Queued,
            cwd: run_cwd,
            original_cwd: Some(original_cwd),
            isolation: input.isolation.unwrap_or(Isolation::None),
            worktree_managed,
            worktree_path,
            agent_dir: agent_dir.display().to_string(),
            command: command.command.clone(),
            args: command.args.clone(),
            timeout_seconds,
            profile: command.profile,
            prompt_strategy: command.prompt_strategy.clone(),
            profile_diagnostics: Some(command.profile_diagnostics.clone()),
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
            result_inspected_at: None,
            transcript_available: false,
            final_result_detected: false,
            partial_result_detected: false,
            transcript_diagnostic: None,
        };

        match launch_task(agent_id.clone(), command, agent_dir, self.tx.clone()).await {
            Ok(active) => {
                record.pid = active.pid;
                transition_status(&mut record, TaskStatus::Running)?;
                record.started_at = Some(now_iso());
                record.updated_at = record.started_at.clone().unwrap();
                self.active.insert(agent_id.clone(), active);
            }
            Err(error) => {
                transition_status(&mut record, TaskStatus::Failed)?;
                record.error = Some(error);
                record.error_type = Some(ErrorType::ProviderStartError);
                record.completed_at = Some(now_iso());
                record.updated_at = record.completed_at.clone().unwrap();
            }
        }
        self.registry.tasks.insert(agent_id.clone(), record);
        self.save().await?;
        Ok(public_task(self.registry.tasks.get(&agent_id).unwrap()))
    }

    async fn stop(&mut self, agent_id: &str) -> Result<Value, String> {
        let active = self.active.remove(agent_id);
        let task = self.require_agent_mut(agent_id)?;
        if active.is_none() {
            if is_final(task.status) {
                return Ok(public_task(task));
            }
            return Err(format!("agent is not running: {agent_id}"));
        }
        transition_status(task, TaskStatus::Stopped)?;
        task.error_type = Some(ErrorType::Stopped);
        task.updated_at = now_iso();
        append_transcript_event(
            &PathBuf::from(&task.agent_dir).join("transcript.jsonl"),
            task.provider,
            "lifecycle",
            "lifecycle",
            "",
            json!({"phase": "stopped"}),
            &provider_env_redactions(task.provider),
        )
        .await;
        let public = public_task(task);
        self.save().await?;
        if let Some(mut active) = active {
            if let Some(pid) = active.pid {
                terminate_child_tree(pid, libc::SIGTERM);
            }
            if let Some(cancel) = active.cancel.take() {
                let _ = cancel.send(());
            }
        }
        Ok(public)
    }

    async fn inspect_result(&mut self, agent_id: &str) -> Result<TaskRecord, String> {
        let mut changed = false;
        let task = self.require_agent_mut(agent_id)?;
        if is_final(task.status) && task.result_inspected_at.is_none() {
            task.result_inspected_at = Some(now_iso());
            changed = true;
        }
        let task = task.clone();
        if changed {
            self.save().await?;
        }
        Ok(task)
    }

    async fn remove(&mut self, agent_id: &str) -> Result<Value, String> {
        let task = self.require_agent_mut(agent_id)?;
        if matches!(task.status, TaskStatus::Running | TaskStatus::Queued) {
            return Err("cannot remove a running agent; stop it first".to_string());
        }
        if task.worktree_managed && task.worktree_path.is_some() {
            let worktree_path = task.worktree_path.clone().unwrap();
            let cleanup_cwd = task
                .original_cwd
                .clone()
                .unwrap_or_else(|| task.cwd.clone());
            run_git(&["worktree", "remove", "-f", &worktree_path], &cleanup_cwd).await?;
        }
        let agent_dir = task.agent_dir.clone();
        transition_status(task, TaskStatus::Removed)?;
        task.updated_at = now_iso();
        self.save().await?;
        let _ = fs::remove_dir_all(agent_dir).await;
        Ok(json!({ "agentId": agent_id, "status": "removed" }))
    }

    async fn shutdown(&mut self) -> Result<(), String> {
        let pids: Vec<u32> = self
            .active
            .values()
            .filter_map(|active| active.pid)
            .collect();
        for pid in &pids {
            terminate_child_tree(*pid, libc::SIGTERM);
        }
        sleep(CHILD_SHUTDOWN_GRACE).await;
        for active in self.active.values_mut() {
            if let Some(pid) = active.pid {
                terminate_child_tree(pid, libc::SIGKILL);
            }
            if let Some(cancel) = active.cancel.take() {
                let _ = cancel.send(());
            }
        }
        Ok(())
    }

    async fn complete(&mut self, completion: TaskCompletion) -> Result<(), String> {
        self.active.remove(&completion.agent_id);
        let task = self.require_agent_mut(&completion.agent_id)?;
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
        append_transcript_event(
            &PathBuf::from(&task.agent_dir).join("transcript.jsonl"),
            task.provider,
            "lifecycle",
            "lifecycle",
            "",
            json!({"phase": "finalized", "status": task.status}),
            &provider_env_redactions(task.provider),
        )
        .await;
        let (transcript_available, final_result_detected, partial_result_detected) =
            transcript_evidence(&task.agent_dir);
        task.transcript_available = transcript_available;
        task.final_result_detected = final_result_detected;
        task.partial_result_detected = partial_result_detected;
        task.transcript_diagnostic = if transcript_available {
            None
        } else {
            Some(json!({
                "failureCategory": "transcript_unavailable",
                "message": "transcript artifact was not available during finalization"
            }))
        };
        let result_path = PathBuf::from(&task.agent_dir).join("result.json");
        let result_json = serde_json::to_vec_pretty(task).map_err(|error| error.to_string())?;
        fs::write(result_path, result_json)
            .await
            .map_err(|error| error.to_string())?;
        self.save().await
    }

    fn require_task(&self, agent_id: &str) -> Result<&TaskRecord, String> {
        self.registry
            .tasks
            .get(agent_id)
            .filter(|task| task.status != TaskStatus::Removed)
            .ok_or_else(|| format!("Unknown agent: {agent_id}"))
    }

    fn require_agent_mut(&mut self, agent_id: &str) -> Result<&mut TaskRecord, String> {
        self.registry
            .tasks
            .get_mut(agent_id)
            .filter(|task| task.status != TaskStatus::Removed)
            .ok_or_else(|| format!("Unknown agent: {agent_id}"))
    }

    fn next_agent_id(&self) -> String {
        loop {
            let agent_id = format!("agent_{}", Uuid::new_v4().simple());
            if !self.registry.tasks.contains_key(&agent_id) {
                return agent_id;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TaskListScope {
    ActiveRecent,
    All,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TaskListInput {
    presentation: Option<bool>,
    scope: Option<TaskListScope>,
    status: Option<Vec<TaskStatus>>,
    provider: Option<Vec<ProviderKind>>,
    mode: Option<Vec<crate::domain::TaskMode>>,
    cwd: Option<String>,
    title_contains: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
    #[serde(rename = "agentId")]
    pub agent_id: String,
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
    pub agent_dir: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub timeout_seconds: i64,
    #[serde(default = "default_launch_profile")]
    pub profile: LaunchProfile,
    #[serde(default)]
    pub prompt_strategy: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_diagnostics: Option<Value>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_inspected_at: Option<String>,
    #[serde(default)]
    pub transcript_available: bool,
    #[serde(default)]
    pub final_result_detected: bool,
    #[serde(default)]
    pub partial_result_detected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_diagnostic: Option<Value>,
}

fn default_launch_profile() -> LaunchProfile {
    LaunchProfile::Bridge
}

struct ActiveTask {
    pid: Option<u32>,
    cancel: Option<oneshot::Sender<()>>,
}

struct TaskCompletion {
    agent_id: String,
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
    agent_id: String,
    command: ProviderCommand,
    agent_dir: PathBuf,
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
                agent_id,
                command,
                claude_command,
                socket_path,
                agent_dir,
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
                stdout_path,
                transcript_path.clone(),
                command.provider,
                "stdout",
                redactions.clone(),
            ))
        }),
        stderr: child.stderr.take().map(|stderr| {
            tokio::spawn(drain_log(
                stderr,
                stderr_path,
                transcript_path,
                command.provider,
                "stderr",
                redactions,
            ))
        }),
    };
    tokio::spawn(async move {
        let completion = wait_for_child(
            agent_id,
            pid,
            command.timeout_seconds,
            child,
            command,
            agent_dir,
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

async fn complete_host_response(
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
    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        let (kind, parsed) = parse_transcript_line(line);
        append_transcript_event(
            &transcript_path,
            command.provider,
            "stdout",
            kind,
            line,
            parsed,
            &redactions,
        )
        .await;
    }
    for line in stderr.lines().filter(|line| !line.trim().is_empty()) {
        let (kind, parsed) = parse_transcript_line(line);
        append_transcript_event(
            &transcript_path,
            command.provider,
            "stderr",
            kind,
            line,
            parsed,
            &redactions,
        )
        .await;
    }
    let success = failure_category.is_none() && result.is_some();
    if success {
        TaskCompletion {
            agent_id,
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
            agent_id,
            status: TaskStatus::Failed,
            exit_code,
            signal: signal.clone(),
            error: Some(category.clone()),
            error_type: Some(if category == "provider_timeout" {
                ErrorType::Timeout
            } else {
                ErrorType::ProviderExitError
            }),
            diagnostic: Some(agent_diagnostic(
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

fn codex_denial_detected(command: &ProviderCommand, stderr: &[u8]) -> bool {
    if command_provider_hint(command) != ProviderKind::Codex {
        return false;
    }
    codex_denial_text(stderr)
}

fn codex_denial_text(stderr: &[u8]) -> bool {
    let text = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    let mentions_patch_rejection = text.contains("patch rejected");
    let mentions_outside_workspace = text.contains("outside of the project")
        || text.contains("outside the project")
        || text.contains("outside of the workspace")
        || text.contains("outside the workspace")
        || text.contains("out-of-workspace");
    let mentions_sandbox_denial = text.contains("sandbox denied")
        || text.contains("sandbox denial")
        || text.contains("sandbox permission")
        || text.contains("sandbox permissions")
        || text.contains("sandbox policy");
    let mentions_approval_denial = text.contains("approval denied")
        || text.contains("approval denial")
        || text.contains("rejected by approval")
        || text.contains("rejected by user approval")
        || text.contains("user approval settings")
        || text.contains("user denied approval");

    mentions_patch_rejection
        || mentions_outside_workspace
        || mentions_sandbox_denial
        || mentions_approval_denial
}

async fn wait_for_child(
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
                    Ok(result) => result,
                    Err(_) => {
                        terminate_child_tree(pid, libc::SIGKILL);
                        (&mut wait).await
                    }
                }
                .map_err(|error| error.to_string());
            }
            _ = sleep(Duration::from_millis(50)), if command_provider_hint(&command) == ProviderKind::Codex => {
                let stderr = fs::read(&stderr_path).await.unwrap_or_default();
                if codex_denial_text(&stderr) {
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
    match output {
        Ok(status) if status.success() => {
            if command_provider_hint(&command) == ProviderKind::Codex || fatal_denial {
                let stdout = std::fs::read(agent_dir.join("stdout.log")).unwrap_or_default();
                let stderr = std::fs::read(agent_dir.join("stderr.log")).unwrap_or_default();
                if fatal_denial || codex_denial_detected(&command, &stderr) {
                    return codex_denial_completion(
                        agent_id,
                        &command,
                        timeout_seconds,
                        status.code(),
                        signal_name(&status),
                        &stdout,
                        &stderr,
                    );
                }
            }
            if command_provider_hint(&command) == ProviderKind::Claude {
                let stdout = std::fs::read(agent_dir.join("stdout.log")).unwrap_or_default();
                let stderr = std::fs::read(agent_dir.join("stderr.log")).unwrap_or_default();
                if !claude_output_is_parseable(&stdout) {
                    return TaskCompletion {
                        agent_id,
                        status: TaskStatus::Failed,
                        exit_code: status.code(),
                        signal: signal_name(&status),
                        error: Some("claude provider output was not parseable".to_string()),
                        error_type: Some(ErrorType::ProviderOutputError),
                        diagnostic: Some(agent_diagnostic(
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
                agent_id,
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
            let stdout = std::fs::read(agent_dir.join("stdout.log")).unwrap_or_default();
            let stderr = std::fs::read(agent_dir.join("stderr.log")).unwrap_or_default();
            if fatal_denial || codex_denial_detected(&command, &stderr) {
                return codex_denial_completion(
                    agent_id,
                    &command,
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

fn codex_denial_completion(
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
            "provider_sandbox_denied",
            timeout_seconds * 1000,
            exit_code,
            signal,
            stdout,
            stderr,
        )),
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

fn agent_diagnostic(
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

fn diagnostic_redactions(command: &ProviderCommand) -> Vec<String> {
    let mut redactions = command.redactions.clone();
    redactions.extend(provider_env_redactions(command.provider));
    redactions
}

fn provider_env_redactions(provider: ProviderKind) -> Vec<String> {
    provider::provider_env(provider)
        .into_iter()
        .filter(|(key, _)| key.contains("KEY") || key.contains("TOKEN") || key.contains("SECRET"))
        .map(|(_, value)| value)
        .filter(|value| !value.is_empty())
        .collect()
}

fn command_kind(command: &ProviderCommand) -> String {
    command
        .command_kind
        .as_deref()
        .unwrap_or(command.provider.as_str())
        .to_string()
}

fn launch_strategy(provider: ProviderKind) -> &'static str {
    if provider != ProviderKind::Claude {
        return "direct";
    }
    if crate::claude_host::socket_path_from_env().is_some() {
        "host_runner"
    } else {
        "host_runner_required"
    }
}

fn command_path(command: &ProviderCommand) -> String {
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

async fn drain_log(
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

fn parse_transcript_line(line: &str) -> (&'static str, Value) {
    let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
        return ("provider_event", json!({}));
    };
    let kind = if value.get("type").and_then(Value::as_str) == Some("result")
        && value.get("result").and_then(Value::as_str).is_some()
    {
        "provider_result"
    } else {
        "provider_event"
    };
    (kind, value)
}

async fn append_transcript_event(
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

fn redact_value(value: Value, redactions: &[String]) -> Value {
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

fn redact_text(text: &str, redactions: &[String]) -> String {
    let mut output = text.to_string();
    for redaction in redactions.iter().filter(|value| !value.is_empty()) {
        output = output.replace(redaction, "<redacted>");
    }
    output
}

async fn read_transcript(task: &TaskRecord, cursor: usize, limit: usize) -> Result<Value, String> {
    let path = PathBuf::from(&task.agent_dir).join("transcript.jsonl");
    if !path.exists() {
        return Ok(json!({
            "agentId": task.agent_id,
            "available": false,
            "events": [],
            "nextCursor": cursor,
            "message": "transcript not available"
        }));
    }
    let text = fs::read_to_string(&path)
        .await
        .map_err(|error| error.to_string())?;
    let all_lines: Vec<&str> = text.lines().collect();
    let max_events = limit.clamp(1, 500);
    let mut events = Vec::new();
    for (index, line) in all_lines.iter().enumerate().skip(cursor).take(max_events) {
        let mut event: Value =
            serde_json::from_str(line).unwrap_or_else(|_| json!({"kind": "malformed"}));
        event = redact_value(event, &provider_env_redactions(task.provider));
        event["index"] = json!(index);
        events.push(event);
    }
    let next_cursor = (cursor + events.len()).min(all_lines.len());
    Ok(json!({
        "agentId": task.agent_id,
        "available": true,
        "events": events,
        "nextCursor": next_cursor,
        "truncated": next_cursor < all_lines.len()
    }))
}

fn transcript_evidence(agent_dir: &str) -> (bool, bool, bool) {
    let path = PathBuf::from(agent_dir).join("transcript.jsonl");
    let Ok(text) = std::fs::read_to_string(path) else {
        return (false, false, false);
    };
    let mut has_event = false;
    let mut has_provider_output = false;
    let mut has_result = false;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        has_event = true;
        let kind = value.get("kind").and_then(Value::as_str);
        let source = value.get("source").and_then(Value::as_str);
        if matches!(source, Some("stdout" | "stderr")) {
            has_provider_output = true;
        }
        if kind == Some("provider_result") {
            has_result = true;
        }
    }
    (has_event, has_result, has_provider_output && !has_result)
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

fn list_tasks(registry: &Registry, arguments: Value) -> Result<Value, String> {
    let arguments = if arguments.is_null() {
        json!({})
    } else {
        arguments
    };
    let input: TaskListInput =
        serde_json::from_value(arguments).map_err(|error| error.to_string())?;
    if input.limit.is_some_and(|limit| !(1..=100).contains(&limit)) {
        return Err("limit must be between 1 and 100".to_string());
    }

    let explicit_scope = input.scope;
    let presentation = input.presentation.unwrap_or(true);
    let scope = explicit_scope.unwrap_or(if presentation {
        TaskListScope::ActiveRecent
    } else {
        TaskListScope::All
    });
    let limit = input
        .limit
        .or_else(|| (presentation && scope == TaskListScope::ActiveRecent).then_some(25));
    let mut tasks: Vec<&TaskRecord> = registry
        .tasks
        .values()
        .filter(|task| task.status != TaskStatus::Removed)
        .filter(|task| agent_matches_list_filters(task, &input))
        .collect();

    if presentation || scope == TaskListScope::ActiveRecent {
        tasks.sort_by(compare_for_presentation_list);
    }
    if let Some(limit) = limit {
        tasks.truncate(limit);
    }

    Ok(json!({
        "tasks": tasks.into_iter().map(public_task).collect::<Vec<_>>(),
        "presentation": presentation,
        "scope": match scope {
            TaskListScope::ActiveRecent => "active_recent",
            TaskListScope::All => "all",
        },
        "limit": limit
    }))
}

fn agent_matches_list_filters(task: &TaskRecord, input: &TaskListInput) -> bool {
    if let Some(statuses) = input.status.as_ref()
        && !statuses.contains(&task.status)
    {
        return false;
    }
    if let Some(providers) = input.provider.as_ref()
        && !providers.contains(&task.provider)
    {
        return false;
    }
    if let Some(modes) = input.mode.as_ref()
        && !modes.contains(&task.mode)
    {
        return false;
    }
    if let Some(cwd) = input.cwd.as_deref()
        && !agent_matches_cwd(task, cwd)
    {
        return false;
    }
    if let Some(title) = input.title_contains.as_deref() {
        let needle = title.to_ascii_lowercase();
        let haystack = display_title(task).to_ascii_lowercase();
        if !haystack.contains(&needle) {
            return false;
        }
    }
    true
}

fn agent_matches_cwd(task: &TaskRecord, cwd: &str) -> bool {
    if task.cwd == cwd || task.original_cwd.as_deref() == Some(cwd) {
        return true;
    }
    let Ok(canonical) = Path::new(cwd).canonicalize() else {
        return false;
    };
    let canonical = canonical.display().to_string();
    task.cwd == canonical || task.original_cwd.as_deref() == Some(canonical.as_str())
}

fn compare_for_presentation_list(left: &&TaskRecord, right: &&TaskRecord) -> Ordering {
    match (is_final(left.status), is_final(right.status)) {
        (false, true) => Ordering::Less,
        (true, false) => Ordering::Greater,
        _ => right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.agent_id.cmp(&right.agent_id)),
    }
}

struct TranscriptProgressSnapshot {
    last_event_at: Option<String>,
    last_output_at: Option<String>,
}

fn agent_progress(task: &TaskRecord) -> Value {
    let now = Utc::now();
    let cadence = provider::output_cadence(task.provider);
    let recommended_poll_ms = cadence_i64(&cadence, "recommendedPollMs", 30_000);
    let recommended_silent_budget_ms = cadence_i64(&cadence, "recommendedSilentBudgetMs", 120_000);
    let fallback_after_ms = cadence_i64(&cadence, "fallbackAfterMs", 180_000);
    let timeout_ms = (task.timeout_seconds > 0).then_some(task.timeout_seconds * 1000);
    let effective_silent_budget_ms = timeout_ms
        .map(|value| value.min(recommended_silent_budget_ms))
        .unwrap_or(recommended_silent_budget_ms);
    let start_at = task
        .started_at
        .as_deref()
        .unwrap_or(task.created_at.as_str());
    let elapsed_ms = millis_since(start_at, now).unwrap_or(0).max(0);
    let transcript = transcript_progress_snapshot(task);
    let last_event_at = transcript
        .last_event_at
        .clone()
        .or_else(|| Some(task.updated_at.clone()));
    let last_output_at = transcript.last_output_at.clone();
    let silent_for_ms = last_output_at
        .as_deref()
        .and_then(|timestamp| millis_since(timestamp, now))
        .unwrap_or(elapsed_ms)
        .max(0);
    let until_next_poll = recommended_poll_ms - (elapsed_ms % recommended_poll_ms.max(1));
    let seconds_until_recommended_check = (until_next_poll.max(0) + 999) / 1000;
    let timeout_remaining_ms = timeout_ms.map(|timeout_ms| timeout_ms - elapsed_ms);
    let final_task = is_final(task.status);
    let stall_risk = if final_task {
        "none"
    } else if timeout_remaining_ms.is_some_and(|remaining| remaining <= 30_000)
        || silent_for_ms >= fallback_after_ms
    {
        "high"
    } else if silent_for_ms >= effective_silent_budget_ms {
        "medium"
    } else {
        "low"
    };

    json!({
        "elapsedMs": elapsed_ms,
        "lastEventAt": last_event_at,
        "lastOutputAt": last_output_at,
        "silentForMs": silent_for_ms,
        "expectedOutputCadence": cadence,
        "recommendedPollMs": recommended_poll_ms,
        "recommendedSilentBudgetMs": recommended_silent_budget_ms,
        "effectiveSilentBudgetMs": effective_silent_budget_ms,
        "fallbackAfterMs": fallback_after_ms,
        "secondsUntilRecommendedCheck": if final_task { 0 } else { seconds_until_recommended_check },
        "stallRisk": stall_risk,
        "timeoutRemainingMs": timeout_remaining_ms,
        "noFurtherPollingNeeded": final_task,
        "recommendedNextTool": if final_task { "agent_result" } else { "agent_observe" }
    })
}

fn cadence_i64(cadence: &Value, key: &str, default: i64) -> i64 {
    cadence.get(key).and_then(Value::as_i64).unwrap_or(default)
}

fn millis_since(timestamp: &str, now: chrono::DateTime<Utc>) -> Option<i64> {
    let then = chrono::DateTime::parse_from_rfc3339(timestamp)
        .ok()?
        .with_timezone(&Utc);
    Some((now - then).num_milliseconds())
}

fn transcript_progress_snapshot(task: &TaskRecord) -> TranscriptProgressSnapshot {
    let path = PathBuf::from(&task.agent_dir).join("transcript.jsonl");
    let Ok(mut file) = std::fs::File::open(path) else {
        return TranscriptProgressSnapshot {
            last_event_at: None,
            last_output_at: None,
        };
    };
    let Ok(size) = file.seek(SeekFrom::End(0)) else {
        return TranscriptProgressSnapshot {
            last_event_at: None,
            last_output_at: None,
        };
    };
    let start = size.saturating_sub(PROGRESS_TRANSCRIPT_TAIL_BYTES);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return TranscriptProgressSnapshot {
            last_event_at: None,
            last_output_at: None,
        };
    }
    let mut text = String::new();
    if file.read_to_string(&mut text).is_err() {
        return TranscriptProgressSnapshot {
            last_event_at: None,
            last_output_at: None,
        };
    }
    if start > 0
        && let Some(index) = text.find('\n')
    {
        text = text[index + 1..].to_string();
    }
    let mut snapshot = TranscriptProgressSnapshot {
        last_event_at: None,
        last_output_at: None,
    };
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let timestamp = value
            .get("ts")
            .or_else(|| value.get("timestamp"))
            .or_else(|| value.get("at"))
            .and_then(Value::as_str)
            .map(str::to_string);
        if let Some(timestamp) = timestamp {
            snapshot.last_event_at = Some(timestamp.clone());
            if value
                .get("source")
                .and_then(Value::as_str)
                .is_some_and(|source| matches!(source, "stdout" | "stderr" | "provider"))
            {
                snapshot.last_output_at = Some(timestamp);
            }
        }
    }
    snapshot
}

fn observe_payload(task: TaskRecord, transcript: Value, timed_out: bool) -> Value {
    let public = public_task(&task);
    json!({
        "agentId": task.agent_id,
        "status": task.status,
        "isFinal": is_final(task.status),
        "agent": public,
        "presentation": public["presentation"],
        "progress": public["progress"],
        "events": transcript["events"],
        "nextCursor": transcript["nextCursor"],
        "timedOut": timed_out,
        "nextActions": public["nextActions"]
    })
}

fn public_task(task: &TaskRecord) -> Value {
    let is_final = is_final(task.status);
    let phase = match task.status {
        TaskStatus::Queued => TaskPhase::Pending,
        TaskStatus::Running => TaskPhase::Active,
        _ => TaskPhase::Done,
    };
    let progress = agent_progress(task);
    json!({
        "agentId": task.agent_id,
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
        "errorType": task.error_type,
        "profile": task.profile,
        "promptStrategy": task.prompt_strategy,
        "profileDiagnostics": task.profile_diagnostics,
        "transcriptDiagnostic": task.transcript_diagnostic,
        "progress": progress,
        "presentation": presentation(task, &progress),
        "nextActions": next_actions(task, &progress)
    })
}

fn presentation(task: &TaskRecord, progress: &Value) -> Value {
    let changed_files = task.changed_files.clone().unwrap_or_default();
    let git_status = task.git_status.clone().unwrap_or_default();
    let has_changes = !changed_files.is_empty() || !git_status.trim().is_empty();
    json!({
        "displayTitle": display_title(task),
        "subtitle": format!("{} {}", task.provider.as_str(), task.mode.as_str()),
        "phase": match task.status {
            TaskStatus::Queued => "pending",
            TaskStatus::Running => "active",
            _ => "done",
        },
        "statusTone": status_tone(task.status),
        "workspace": task.cwd,
        "timestamps": {
            "createdAt": task.created_at,
            "updatedAt": task.updated_at,
            "startedAt": task.started_at,
            "completedAt": task.completed_at,
        },
        "durationMs": duration_ms(task),
        "errorType": task.error_type,
        "result": {
            "available": is_final(task.status),
            "hasChanges": has_changes,
            "changedFileCount": changed_files.len(),
            "inspectedAt": task.result_inspected_at,
            "transcriptAvailable": task.transcript_available,
            "finalResultDetected": task.final_result_detected,
            "partialResultDetected": task.partial_result_detected,
            "reviewPacketAvailable": is_final(task.status)
        },
        "progress": progress,
        "verificationStatus": "not_verified",
        "actions": presentation_actions(task),
        "nextActions": next_actions(task, progress)
    })
}

fn display_title(task: &TaskRecord) -> String {
    task.title
        .clone()
        .unwrap_or_else(|| format!("{} {} task", task.provider.as_str(), task.mode.as_str()))
}

fn status_tone(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Queued => "pending",
        TaskStatus::Running => "running",
        TaskStatus::Succeeded => "success",
        TaskStatus::Failed => "error",
        TaskStatus::Stopped => "stopped",
        TaskStatus::FailedStale => "stale",
        TaskStatus::Removed => "removed",
    }
}

fn presentation_actions(task: &TaskRecord) -> Value {
    let mut actions = vec![
        action(
            "observe",
            Some("agent_observe"),
            action_state(!is_final(task.status)),
            unavailable_reason(is_final(task.status), "agent_final"),
        ),
        action(
            "wait",
            Some("agent_wait"),
            action_state(!is_final(task.status)),
            None,
        ),
        action("inspect_status", Some("agent_status"), "available", None),
        action("inspect_logs", Some("agent_logs"), "available", None),
        action(
            "inspect_transcript",
            Some("agent_transcript"),
            action_state(task.transcript_available),
            unavailable_reason(!task.transcript_available, "transcript_unavailable"),
        ),
        action(
            "inspect_result",
            Some("agent_result"),
            action_state(is_final(task.status)),
            unavailable_reason(!is_final(task.status), "agent_not_final"),
        ),
        action(
            "stop",
            Some("agent_stop"),
            action_state(matches!(
                task.status,
                TaskStatus::Queued | TaskStatus::Running
            )),
            unavailable_reason(
                !matches!(task.status, TaskStatus::Queued | TaskStatus::Running),
                "agent_not_running",
            ),
        ),
    ];

    let cleanup_state = if !is_final(task.status) {
        "unavailable"
    } else if task.worktree_managed && task.result_inspected_at.is_none() {
        "unsafe"
    } else {
        "available"
    };
    let cleanup_reason = if !is_final(task.status) {
        Some("agent_not_final")
    } else if task.worktree_managed && task.result_inspected_at.is_none() {
        Some("managed_worktree_cleanup_requires_result_inspection")
    } else {
        None
    };
    actions.push(action(
        "cleanup",
        Some("agent_remove"),
        cleanup_state,
        cleanup_reason,
    ));
    actions.push(action(
        "reply",
        None,
        "unavailable",
        Some("provider_agent_not_interactive"),
    ));
    actions.push(action(
        "resume",
        None,
        "unavailable",
        Some("provider_agent_not_resumable"),
    ));
    Value::Array(actions)
}

fn action_state(available: bool) -> &'static str {
    if available {
        "available"
    } else {
        "unavailable"
    }
}

fn unavailable_reason(unavailable: bool, reason: &'static str) -> Option<&'static str> {
    unavailable.then_some(reason)
}

fn action(id: &str, tool: Option<&str>, state: &str, reason: Option<&str>) -> Value {
    let mut value = json!({
        "id": id,
        "tool": tool,
        "state": state
    });
    if let Some(reason) = reason {
        value["reason"] = Value::String(reason.to_string());
    }
    value
}

fn next_actions(task: &TaskRecord, progress: &Value) -> Value {
    let mut actions = Vec::new();
    if !is_final(task.status) {
        let recommended_poll_ms = progress["recommendedPollMs"].as_i64().unwrap_or(30_000);
        let stall_risk = progress["stallRisk"].as_str().unwrap_or("low");
        actions.push(next_action(
            "observe",
            Some("agent_observe"),
            json!({ "agentId": task.agent_id, "cursor": 0, "limit": 100, "timeoutMs": recommended_poll_ms }),
            "available",
            "Observe bounded transcript and lifecycle progress before deciding whether to wait, inspect, stop, or fall back.",
            "safe",
        ));
        actions.push(next_action(
            "wait",
            Some("agent_wait"),
            json!({ "agentId": task.agent_id, "timeoutMs": recommended_poll_ms.min(MAX_WAIT_MS) }),
            "available",
            "Wait for finalization using the provider-aware polling interval.",
            "safe",
        ));
        actions.push(next_action(
            "inspect_logs",
            Some("agent_logs"),
            json!({ "agentId": task.agent_id }),
            "available",
            if stall_risk == "high" { "Inspect logs because the agent has exceeded its recommended observation budget or is near timeout." } else { "Inspect incremental stdout and stderr if observation times out or progress is unclear." },
            "safe",
        ));
        actions.push(next_action(
            "inspect_status",
            Some("agent_status"),
            json!({ "agentId": task.agent_id }),
            "available",
            "Confirm the task lifecycle state without assuming provider completion verifies the original request.",
            "safe",
        ));
        actions.push(next_action(
            "stop",
            Some("agent_stop"),
            json!({ "agentId": task.agent_id }),
            "available",
            if stall_risk == "high" { "Stop only after deciding the agent is no longer useful; stopped agents remain inspectable." } else { "Stop only when the agent is no longer useful; provider silence within the observation budget is not enough by itself." },
            "unsafe",
        ));
        return Value::Array(actions);
    }

    if task.result_inspected_at.is_none() {
        actions.push(next_action(
            "inspect_result",
            Some("agent_result"),
            json!({ "agentId": task.agent_id }),
            "available",
            "Inspect final logs, diagnostics, git state, transcript evidence, and review packet before cleanup or verification.",
            "safe",
        ));
        if task.worktree_managed {
            actions.push(next_action(
                "cleanup",
                Some("agent_remove"),
                json!({ "agentId": task.agent_id }),
                "unsafe",
                "Managed worktree cleanup requires explicit final result inspection first.",
                "unsafe",
            ));
        }
        return Value::Array(actions);
    }

    if matches!(
        task.status,
        TaskStatus::Failed | TaskStatus::Stopped | TaskStatus::FailedStale
    ) || task.error_type.is_some()
    {
        actions.push(next_action(
            "inspect_logs",
            Some("agent_logs"),
            json!({ "agentId": task.agent_id }),
            "available",
            "Inspect logs and diagnostics before deciding whether to rerun, narrow the prompt, or continue manually.",
            "safe",
        ));
        if task.transcript_available {
            actions.push(next_action(
                "inspect_transcript",
                Some("agent_transcript"),
                json!({ "agentId": task.agent_id }),
                "available",
                "Inspect transcript evidence when provider behavior or final-state classification is unclear.",
                "safe",
            ));
        }
    } else {
        actions.push(next_action(
            "verify_project",
            None,
            json!({}),
            "available",
            "Run the relevant project verification before claiming the original request is complete.",
            "requires_verification",
        ));
    }

    if task.worktree_managed {
        actions.push(next_action(
            "cleanup",
            Some("agent_remove"),
            json!({ "agentId": task.agent_id }),
            "available",
            "Remove the managed worktree only after inspecting the result and preserving any needed changes.",
            "destructive",
        ));
    }

    Value::Array(actions)
}

fn next_action(
    id: &str,
    tool: Option<&str>,
    arguments: Value,
    state: &str,
    reason: &str,
    safety: &str,
) -> Value {
    json!({
        "id": id,
        "tool": tool,
        "arguments": arguments,
        "state": state,
        "reason": reason,
        "safety": safety
    })
}

fn review_packet(task: &TaskRecord, stdout_truncated: bool, stderr_truncated: bool) -> Value {
    let is_final = is_final(task.status);
    let progress = agent_progress(task);
    let git_status = task.git_status.clone().unwrap_or_default();
    let changed_files = task.changed_files.clone().unwrap_or_default();
    let has_changes = !changed_files.is_empty() || !git_status.trim().is_empty();
    json!({
        "agentId": task.agent_id,
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
        "profile": task.profile,
        "profileDiagnostics": task.profile_diagnostics,
        "transcriptAvailable": task.transcript_available,
        "finalResultDetected": task.final_result_detected,
        "partialResultDetected": task.partial_result_detected,
        "transcriptDiagnostic": task.transcript_diagnostic,
        "stdoutTruncated": stdout_truncated,
        "stderrTruncated": stderr_truncated,
        "progress": progress,
        "nextActions": next_actions(task, &progress),
        "recommendedActions": recommended_actions(task, has_changes)
    })
}

fn recommended_actions(task: &TaskRecord, has_changes: bool) -> Vec<&'static str> {
    if !is_final(task.status) {
        return vec![
            "Use agent_observe with a bounded timeout before treating silence as a stall.",
            "Use agent_logs with line cursors to inspect incremental output.",
            "Use agent_status to confirm whether the agent is still active.",
            "Use agent_stop if the agent is no longer useful.",
        ];
    }

    if task.error_type == Some(ErrorType::CodexSandboxDenied) {
        return vec![
            "Inspect task logs, stderr, and diagnostic metadata for the exact Codex denial reason.",
            "Inspect cwd and workspace policy before retrying.",
            "Inspect prompt scope and confirm it does not request changes outside the project.",
            "Inspect isolation strategy; prefer managed worktree isolation for write-capable retries.",
            "Do not silently relax sandbox permissions or blindly retry without understanding the cause.",
        ];
    }

    let mut actions =
        vec!["Inspect stdout, stderr, diagnostics, git status, diff, and changed files."];
    if task.transcript_available {
        actions.push("Inspect agent_transcript when provider behavior or final-state classification is unclear.");
    }
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
        actions.push("Call agent_remove only after inspecting the managed worktree result.");
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
        Ok(text) => parse_registry_text(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Registry {
            tasks: BTreeMap::new(),
        }),
        Err(error) => Err(error.to_string()),
    }
}

pub(crate) fn validate_registry_text(text: &str) -> Result<(), String> {
    parse_registry_text(text).map(|_| ())
}

fn parse_registry_text(text: &str) -> Result<Registry, String> {
    let mut value: Value = serde_json::from_str(text)
        .map_err(|error| format!("failed to parse registry.json: {error}"))?;
    normalize_legacy_registry_fields(&mut value);
    serde_json::from_value(value).map_err(|error| format!("failed to parse registry.json: {error}"))
}

fn normalize_legacy_registry_fields(value: &mut Value) {
    let Some(tasks) = value.get_mut("tasks").and_then(Value::as_object_mut) else {
        return;
    };
    for task in tasks.values_mut() {
        let Some(record) = task.as_object_mut() else {
            continue;
        };
        if !record.contains_key("agentId")
            && let Some(task_id) = record.get("taskId").cloned()
        {
            record.insert("agentId".to_string(), task_id);
        }
        if !record.contains_key("agentDir")
            && let Some(task_dir) = record.get("taskDir").cloned()
        {
            record.insert("agentDir".to_string(), task_dir);
        }
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

fn normalize_observe_ms(value: Option<i64>) -> i64 {
    value.unwrap_or(30_000).clamp(0, MAX_OBSERVE_MS)
}

fn normalize_observe_limit(value: Option<u64>) -> usize {
    value.unwrap_or(100).clamp(1, MAX_OBSERVE_EVENTS as u64) as usize
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

#[cfg(unix)]
fn configure_child_process_group(command: &mut ProcessCommand) {
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
fn configure_child_process_group(_command: &mut ProcessCommand) {}

#[cfg(unix)]
fn terminate_child_tree(pid: u32, signal: i32) {
    unsafe {
        libc::killpg(pid as libc::pid_t, signal);
    }
}

#[cfg(not(unix))]
fn terminate_child_tree(_pid: u32, _signal: i32) {}

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
            agent_id: "agent_11111111111111111111111111111111".to_string(),
            provider: ProviderKind::Codex,
            mode: crate::domain::TaskMode::Review,
            title: None,
            status,
            cwd: ".".to_string(),
            original_cwd: None,
            isolation: Isolation::None,
            worktree_managed: false,
            worktree_path: None,
            agent_dir: ".".to_string(),
            command: String::new(),
            args: Vec::new(),
            timeout_seconds: 1,
            profile: LaunchProfile::Bridge,
            prompt_strategy: "bridge".to_string(),
            profile_diagnostics: None,
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
            result_inspected_at: None,
            transcript_available: false,
            final_result_detected: false,
            partial_result_detected: false,
            transcript_diagnostic: None,
        }
    }

    fn action_state<'a>(task: &'a Value, id: &str) -> &'a str {
        task["presentation"]["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|action| action["id"] == id)
            .unwrap_or_else(|| panic!("missing action: {id}"))["state"]
            .as_str()
            .unwrap()
    }

    fn action_reason<'a>(task: &'a Value, id: &str) -> Option<&'a str> {
        task["presentation"]["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|action| action["id"] == id)
            .unwrap_or_else(|| panic!("missing action: {id}"))["reason"]
            .as_str()
    }

    fn next_action(task: &Value, index: usize) -> &Value {
        &task["presentation"]["nextActions"].as_array().unwrap()[index]
    }

    #[test]
    fn transition_status_rejects_illegal_moves() {
        let mut task = sample_task(TaskStatus::Queued);
        transition_status(&mut task, TaskStatus::Running).unwrap();
        transition_status(&mut task, TaskStatus::Succeeded).unwrap();

        assert!(transition_status(&mut task, TaskStatus::Running).is_err());
    }

    #[test]
    fn public_agent_includes_presentation_for_all_lifecycle_states() {
        for (status, phase, tone) in [
            (TaskStatus::Queued, "pending", "pending"),
            (TaskStatus::Running, "active", "running"),
            (TaskStatus::Succeeded, "done", "success"),
            (TaskStatus::Failed, "done", "error"),
            (TaskStatus::Stopped, "done", "stopped"),
            (TaskStatus::FailedStale, "done", "stale"),
        ] {
            let mut task = sample_task(status);
            task.title = Some("Presentation audit".to_string());
            task.transcript_available = true;
            task.final_result_detected = status == TaskStatus::Succeeded;
            task.partial_result_detected = status == TaskStatus::Failed;
            task.changed_files = Some(vec!["README.md".to_string()]);
            if status == TaskStatus::FailedStale {
                task.error_type = Some(ErrorType::Stale);
            }

            let public = public_task(&task);

            assert_eq!(public["presentation"]["displayTitle"], "Presentation audit");
            assert_eq!(public["presentation"]["subtitle"], "codex review");
            assert_eq!(public["presentation"]["phase"], phase);
            assert_eq!(public["presentation"]["statusTone"], tone);
            assert_eq!(public["presentation"]["result"]["changedFileCount"], 1);
            assert_eq!(
                public["presentation"]["result"]["transcriptAvailable"],
                true
            );
            assert_eq!(public["presentation"]["verificationStatus"], "not_verified");
            assert!(public["presentation"]["result"]["available"].is_boolean());
            assert!(public["presentation"]["actions"].is_array());
            assert!(public["presentation"]["nextActions"].is_array());
            assert!(public["nextActions"].is_array());
            assert!(public.get("stdout").is_none());
            assert!(public.get("gitDiff").is_none());
        }
    }

    #[test]
    fn presentation_uses_safe_display_title_fallback() {
        let task = sample_task(TaskStatus::Running);
        let public = public_task(&task);

        assert_eq!(public["presentation"]["displayTitle"], "codex review task");
    }

    #[test]
    fn presentation_actions_reflect_running_final_and_worktree_states() {
        let mut running = sample_task(TaskStatus::Running);
        running.transcript_available = true;
        let running_public = public_task(&running);
        assert_eq!(action_state(&running_public, "wait"), "available");
        assert_eq!(action_state(&running_public, "stop"), "available");
        assert_eq!(
            running_public["presentation"]["actions"]
                .as_array()
                .unwrap()
                .iter()
                .find(|action| action["id"] == "wait")
                .unwrap()["tool"],
            "agent_wait"
        );
        assert_eq!(
            action_state(&running_public, "inspect_result"),
            "unavailable"
        );
        assert_eq!(next_action(&running_public, 0)["id"], "observe");
        assert_eq!(next_action(&running_public, 0)["tool"], "agent_observe");
        assert_eq!(
            next_action(&running_public, 0)["arguments"]["agentId"],
            running.agent_id
        );
        assert_eq!(next_action(&running_public, 0)["safety"], "safe");
        assert_eq!(
            action_reason(&running_public, "inspect_result"),
            Some("agent_not_final")
        );
        assert_eq!(action_state(&running_public, "reply"), "unavailable");
        assert_eq!(
            action_reason(&running_public, "reply"),
            Some("provider_agent_not_interactive")
        );

        let mut final_task = sample_task(TaskStatus::Succeeded);
        final_task.transcript_available = false;
        let final_public = public_task(&final_task);
        assert_eq!(action_state(&final_public, "wait"), "unavailable");
        assert_eq!(action_state(&final_public, "inspect_result"), "available");
        assert_eq!(action_state(&final_public, "cleanup"), "available");
        assert_eq!(
            action_state(&final_public, "inspect_transcript"),
            "unavailable"
        );
        assert_eq!(
            action_reason(&final_public, "inspect_transcript"),
            Some("transcript_unavailable")
        );

        let mut worktree = sample_task(TaskStatus::Succeeded);
        worktree.worktree_managed = true;
        worktree.worktree_path = Some("/tmp/worktree".to_string());
        let worktree_public = public_task(&worktree);
        assert_eq!(action_state(&worktree_public, "cleanup"), "unsafe");
        assert_eq!(next_action(&worktree_public, 0)["id"], "inspect_result");
        assert_eq!(next_action(&worktree_public, 1)["id"], "cleanup");
        assert_eq!(next_action(&worktree_public, 1)["state"], "unsafe");
        assert_eq!(
            action_reason(&worktree_public, "cleanup"),
            Some("managed_worktree_cleanup_requires_result_inspection")
        );

        worktree.result_inspected_at = Some(now_iso());
        let inspected_worktree_public = public_task(&worktree);
        assert_eq!(
            action_state(&inspected_worktree_public, "cleanup"),
            "available"
        );
        assert_eq!(
            next_action(&inspected_worktree_public, 1)["safety"],
            "destructive"
        );
        assert_eq!(action_reason(&inspected_worktree_public, "cleanup"), None);
        assert!(inspected_worktree_public["presentation"]["result"]["inspectedAt"].is_string());

        let mut stale = sample_task(TaskStatus::FailedStale);
        stale.error_type = Some(ErrorType::Stale);
        stale.result_inspected_at = Some(now_iso());
        let stale_public = public_task(&stale);
        assert_eq!(stale_public["presentation"]["phase"], "done");
        assert_eq!(stale_public["presentation"]["errorType"], "stale");
        assert_eq!(next_action(&stale_public, 0)["id"], "inspect_logs");
        assert_eq!(action_state(&stale_public, "resume"), "unavailable");
    }

    #[test]
    fn list_tasks_defaults_to_bounded_active_recent_presentation() {
        let mut registry = Registry {
            tasks: BTreeMap::new(),
        };
        let mut old_final = sample_task(TaskStatus::Succeeded);
        old_final.agent_id = "agent_old".to_string();
        old_final.title = Some("Old final".to_string());
        old_final.updated_at = "2026-06-01T00:00:00.000Z".to_string();
        let mut running = sample_task(TaskStatus::Running);
        running.agent_id = "agent_running".to_string();
        running.title = Some("Running".to_string());
        running.updated_at = "2026-06-01T00:00:01.000Z".to_string();
        let mut recent_final = sample_task(TaskStatus::Succeeded);
        recent_final.agent_id = "agent_recent".to_string();
        recent_final.title = Some("Recent final".to_string());
        recent_final.updated_at = "2026-06-02T00:00:00.000Z".to_string();
        let mut removed = sample_task(TaskStatus::Removed);
        removed.agent_id = "agent_removed".to_string();

        for task in [old_final, running, recent_final, removed] {
            registry.tasks.insert(task.agent_id.clone(), task);
        }

        let listed = list_tasks(&registry, json!({})).unwrap();
        let tasks = listed["tasks"].as_array().unwrap();

        assert_eq!(listed["presentation"], true);
        assert_eq!(listed["scope"], "active_recent");
        assert_eq!(listed["limit"], 25);
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0]["agentId"], "agent_running");
        assert_eq!(tasks[1]["agentId"], "agent_recent");
        assert_eq!(tasks[2]["agentId"], "agent_old");
        assert!(tasks.iter().all(|task| task.get("presentation").is_some()));
    }

    #[test]
    fn list_tasks_filters_and_rejects_invalid_limits() {
        let mut registry = Registry {
            tasks: BTreeMap::new(),
        };
        let mut cursor_review = sample_task(TaskStatus::Succeeded);
        cursor_review.agent_id = "agent_cursor".to_string();
        cursor_review.provider = ProviderKind::Cursor;
        cursor_review.title = Some("Native UX review".to_string());
        cursor_review.cwd = "/repo".to_string();
        let mut codex_command = sample_task(TaskStatus::Running);
        codex_command.agent_id = "agent_codex".to_string();
        codex_command.mode = crate::domain::TaskMode::Command;
        codex_command.title = Some("Other task".to_string());
        codex_command.cwd = "/other".to_string();
        registry
            .tasks
            .insert(cursor_review.agent_id.clone(), cursor_review);
        registry
            .tasks
            .insert(codex_command.agent_id.clone(), codex_command);

        let filtered = list_tasks(
            &registry,
            json!({
                "provider": ["cursor"],
                "mode": ["review"],
                "cwd": "/repo",
                "titleContains": "ux",
                "limit": 1
            }),
        )
        .unwrap();
        let tasks = filtered["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0]["agentId"], "agent_cursor");

        let raw = list_tasks(&registry, json!({"presentation": false, "scope": "all"})).unwrap();
        assert_eq!(raw["presentation"], false);
        assert_eq!(raw["scope"], "all");
        assert_eq!(raw["limit"], Value::Null);
        assert_eq!(raw["tasks"].as_array().unwrap().len(), 2);

        let error = list_tasks(&registry, json!({"limit": 101})).unwrap_err();
        assert!(error.contains("limit"));
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
    async fn load_registry_accepts_legacy_task_id_records() {
        let dir = temp_dir("registry-legacy");
        let legacy_dir = dir.join("tasks").join("task_legacy");
        std::fs::create_dir_all(&legacy_dir).unwrap();
        let mut task = serde_json::to_value(sample_task(TaskStatus::Succeeded)).unwrap();
        let object = task.as_object_mut().unwrap();
        object.insert("taskId".to_string(), json!("task_legacy"));
        object.insert(
            "taskDir".to_string(),
            json!(legacy_dir.display().to_string()),
        );
        object.remove("agentId");
        object.remove("agentDir");
        fs::write(
            dir.join("registry.json"),
            serde_json::to_vec_pretty(&json!({
                "tasks": {
                    "task_legacy": task
                }
            }))
            .unwrap(),
        )
        .await
        .unwrap();

        let registry = load_registry(&dir).await.unwrap();

        let loaded = registry.tasks.get("task_legacy").unwrap();
        assert_eq!(loaded.agent_id, "task_legacy");
        assert_eq!(loaded.agent_dir, legacy_dir.display().to_string());
        assert_eq!(loaded.status, TaskStatus::Succeeded);
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

    #[test]
    fn codex_denial_text_matches_specific_fatal_denial_phrases() {
        for stderr in [
            "patch rejected",
            "Patch rejected: file is outside the workspace",
            "write outside of the project",
            "sandbox denied",
            "sandbox permission blocked command",
            "approval denied",
            "rejected by user approval settings",
        ] {
            assert!(
                codex_denial_text(stderr.as_bytes()),
                "expected fatal Codex denial for: {stderr}"
            );
        }
    }

    #[test]
    fn codex_denial_text_avoids_broad_sandbox_false_positives() {
        for stderr in [
            "sandbox connection denied by proxy",
            "permission denied while reading cache",
            "approval requested",
            "patch failed to apply cleanly",
        ] {
            assert!(
                !codex_denial_text(stderr.as_bytes()),
                "unexpected fatal Codex denial for: {stderr}"
            );
        }
    }

    #[tokio::test]
    async fn terminate_child_tree_sends_sigterm_to_unix_process_group() {
        let mut command = ProcessCommand::new("/bin/sleep");
        command
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        configure_child_process_group(&mut command);
        let mut child = command.spawn().unwrap();
        let pid = child.id().unwrap();

        terminate_child_tree(pid, libc::SIGTERM);
        let status = timeout(Duration::from_secs(3), child.wait())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(signal_name(&status).as_deref(), Some("SIGTERM"));
    }
}
