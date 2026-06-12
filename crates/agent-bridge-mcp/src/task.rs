use crate::domain::{
    ErrorType, FailureCategory, Isolation, LaunchProfile, PartialResult, ProviderKind, RetryPolicy,
    TaskStatus, TimeoutSeconds,
};
use crate::provider::{self, ProviderTask};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tokio::fs;
use tokio::sync::{OnceCell, mpsc, oneshot, watch};
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
/// Hard upper bound on how long to wait for a child to be reaped after SIGKILL.
/// A process the OS cannot reap within this window is reported, never waited on
/// indefinitely.
const SIGKILL_REAP_GRACE: Duration = Duration::from_secs(1);
static MANAGER: OnceCell<TaskManagerHandle> = OnceCell::const_new();

mod supervision;
pub(crate) use supervision::{
    register_active_pid, terminate_all_active_pids, terminate_child_tree, unregister_active_pid,
};

mod registry;
use registry::{load_registry, now_iso, save_registry};

pub(crate) use registry::{normalize_legacy_registry_fields_exported, validate_registry_text};

mod review;
#[allow(unused_imports)]
use review::{
    add_detail, display_title, insert_detail_fields, insert_evidence_fields, insert_outcome_fields,
    is_final, list_tasks, normalize_max_bytes, normalize_observe_limit, normalize_observe_ms,
    normalize_wait_ms, observe_payload, public_task, read_capped_file, read_transcript,
    slice_lines, transcript_evidence, transition_status,
};

mod complete;
use complete::{append_transcript_event, git_snapshot, provider_env_redactions, run_git};

mod spawn;
use spawn::{
    apply_launch_outcome, create_worktree, default_launch_profile, launch_task, safe_cwd,
    validate_spawn_arguments,
};

fn max_active_tasks() -> usize {
    crate::config::Config::from_env(crate::config::ConfigCliOverrides::default())
        .map(|config| config.max_active_tasks())
        .unwrap_or(crate::config::DEFAULT_MAX_ACTIVE_TASKS)
}

#[derive(Clone)]
pub struct TaskManagerHandle {
    tx: mpsc::Sender<ActorCommand>,
}

impl TaskManagerHandle {
    pub async fn from_env() -> Result<Self, String> {
        MANAGER
            .get_or_try_init(|| async {
                let state_dir =
                    crate::config::Config::from_env(crate::config::ConfigCliOverrides::default())?
                        .state_dir()
                        .to_path_buf();
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
        // Any provider child died when the previous server process exited, so a
        // crash leaves Queued/Running records and their managed worktrees
        // orphaned. Reconcile them: mark the record stale and reclaim the
        // worktree.
        for task in registry.tasks.values_mut() {
            if matches!(task.status, TaskStatus::Queued | TaskStatus::Running) {
                transition_status(task, TaskStatus::FailedStale)?;
                task.error = Some(
                    "task was running when the MCP server restarted; resume is not supported in v1"
                        .to_string(),
                );
                task.error_type = Some(ErrorType::Stale);
                task.updated_at = now_iso();
                if task.worktree_managed
                    && let Some(worktree_path) = task.worktree_path.clone()
                {
                    let cleanup_cwd = task
                        .original_cwd
                        .clone()
                        .unwrap_or_else(|| task.cwd.clone());
                    match run_git(&["worktree", "remove", "-f", &worktree_path], &cleanup_cwd).await
                    {
                        Ok(_) => {
                            task.worktree_managed = false;
                        }
                        Err(cleanup_error) => {
                            task.diagnostic = Some(json!({
                                "failureCategory": FailureCategory::WorktreeReclaimFailed.as_str(),
                                "message": format!(
                                    "failed to reclaim orphaned worktree on restart: {cleanup_error}"
                                ),
                                "worktreePath": worktree_path,
                            }));
                        }
                    }
                }
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
            watches: BTreeMap::new(),
            tx: tx.clone(),
        };
        let join = tokio::spawn(actor.run(rx));
        tokio::spawn(async move {
            if let Err(error) = join.await {
                tracing::error!(error = %error, "[agent-bridge] task actor failed: {error}");
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

    /// Lean state-only read (subsumes the former agent_status tool / observe limit:0).
    pub async fn status(&self, agent_id: String, detailed: bool) -> Result<Value, String> {
        let task: TaskRecord = self
            .request(|reply| ActorCommand::Get(agent_id.clone(), reply))
            .await?;
        let mut value = public_task(&task);
        if detailed {
            add_detail(&mut value, &task);
        }
        Ok(value)
    }

    /// Block until finality or timeout (subsumes the former agent_wait tool / observe until:"final").
    pub async fn wait(
        &self,
        agent_id: String,
        timeout_ms: Option<i64>,
        detailed: bool,
    ) -> Result<Value, String> {
        let deadline = Instant::now() + Duration::from_millis(normalize_wait_ms(timeout_ms) as u64);
        loop {
            let mut status = self.status(agent_id.clone(), detailed).await?;
            if status["isFinal"].as_bool().unwrap_or(false) {
                return Ok(status);
            }
            if Instant::now() >= deadline {
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
        detailed: bool,
    ) -> Result<Value, String> {
        let cursor = cursor.unwrap_or(0) as usize;
        let limit = normalize_observe_limit(limit);
        let timeout_ms = normalize_observe_ms(timeout_ms);
        let deadline = Instant::now() + Duration::from_millis(timeout_ms as u64);
        let mut watcher = self.subscribe(agent_id.clone()).await?;
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
                return Ok(observe_payload(task, transcript, false, detailed));
            }
            if Instant::now() >= deadline {
                return Ok(observe_payload(task, transcript, true, detailed));
            }
            if next_cursor > cursor as u64 {
                return Ok(observe_payload(task, transcript, false, detailed));
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(observe_payload(task, transcript, true, detailed));
            }
            let _ = timeout(deadline - now, watcher.changed()).await;
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

    /// Final evidence. The review packet (`summary`) and `changedFiles` are returned by
    /// default; raw `stdout`/`stderr`/`diff`/`transcript` sections are fetched on demand so
    /// large evidence stays out of context until requested (subsumes the former agent_logs tool).
    #[allow(clippy::too_many_arguments)]
    pub async fn result(
        &self,
        agent_id: String,
        sections: ResultSections,
        max_bytes: Option<i64>,
        stdout_line: Option<usize>,
        stderr_line: Option<usize>,
        transcript_cursor: Option<u64>,
        transcript_limit: Option<u64>,
        detailed: bool,
    ) -> Result<Value, String> {
        let task: TaskRecord = self
            .request(|reply| ActorCommand::InspectResult(agent_id.clone(), reply))
            .await?;
        let mut value = public_task(&task);
        insert_outcome_fields(&mut value, &task);
        let (stdout_truncated, stderr_truncated) = if sections.stdout || sections.stderr {
            let logs = self
                .logs(agent_id.clone(), max_bytes, stdout_line, stderr_line)
                .await?;
            if let Some(object) = value.as_object_mut() {
                if sections.stdout {
                    object.insert("stdout".to_string(), logs["stdout"].clone());
                    object.insert(
                        "stdoutTruncated".to_string(),
                        logs["stdoutTruncated"].clone(),
                    );
                    object.insert("nextStdoutLine".to_string(), logs["nextStdoutLine"].clone());
                }
                if sections.stderr {
                    object.insert("stderr".to_string(), logs["stderr"].clone());
                    object.insert(
                        "stderrTruncated".to_string(),
                        logs["stderrTruncated"].clone(),
                    );
                    object.insert("nextStderrLine".to_string(), logs["nextStderrLine"].clone());
                }
            }
            (
                logs["stdoutTruncated"].as_bool().unwrap_or(false),
                logs["stderrTruncated"].as_bool().unwrap_or(false),
            )
        } else {
            (false, false)
        };
        insert_evidence_fields(
            &mut value,
            &task,
            &sections,
            stdout_truncated,
            stderr_truncated,
        );
        if sections.transcript {
            let transcript = read_transcript(
                &task,
                transcript_cursor.unwrap_or(0) as usize,
                transcript_limit.unwrap_or(200) as usize,
            )
            .await?;
            if let Some(object) = value.as_object_mut() {
                object.insert("transcript".to_string(), transcript);
            }
        }
        if detailed {
            insert_detail_fields(&mut value, &task);
        }
        Ok(value)
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

    async fn subscribe(&self, agent_id: String) -> Result<watch::Receiver<u64>, String> {
        self.request(|reply| ActorCommand::Subscribe(agent_id, reply))
            .await
    }
}

/// Evidence sections a caller can request from `agent_result`. The review-packet
/// summary is always returned; these gate the larger, on-demand evidence.
#[derive(Debug, Clone, Copy)]
pub struct ResultSections {
    pub changed_files: bool,
    pub stdout: bool,
    pub stderr: bool,
    pub diff: bool,
    pub transcript: bool,
}

impl ResultSections {
    pub fn default_sections() -> Self {
        Self {
            changed_files: true,
            stdout: false,
            stderr: false,
            diff: false,
            transcript: false,
        }
    }

    pub fn from_names<'a>(names: impl Iterator<Item = &'a str>) -> Self {
        let mut sections = Self {
            changed_files: false,
            stdout: false,
            stderr: false,
            diff: false,
            transcript: false,
        };
        for name in names {
            match name {
                "summary" => {}
                "changedFiles" => sections.changed_files = true,
                "stdout" => sections.stdout = true,
                "stderr" => sections.stderr = true,
                "diff" => sections.diff = true,
                "transcript" => sections.transcript = true,
                _ => {}
            }
        }
        sections
    }
}

enum ActorCommand {
    Spawn(Value, oneshot::Sender<Result<Value, String>>),
    List(Value, oneshot::Sender<Result<Value, String>>),
    Get(String, oneshot::Sender<Result<TaskRecord, String>>),
    Subscribe(
        String,
        oneshot::Sender<Result<watch::Receiver<u64>, String>>,
    ),
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
    watches: BTreeMap<String, watch::Sender<u64>>,
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
                ActorCommand::Get(agent_id, reply) => {
                    let result = self.require_task(&agent_id).cloned();
                    let _ = reply.send(result);
                }
                ActorCommand::Subscribe(agent_id, reply) => {
                    let result = self.subscribe_task(&agent_id);
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
                        tracing::error!(
                            error = %error,
                            "[agent-bridge] failed to complete task: {error}"
                        );
                    }
                }
            }
        }
    }

    #[tracing::instrument(
        name = "spawn_task",
        skip(self, arguments),
        fields(
            agent_id = tracing::field::Empty,
            provider = tracing::field::Empty,
            mode = tracing::field::Empty,
            task_status = "queued"
        )
    )]
    async fn spawn(&mut self, arguments: Value) -> Result<Value, String> {
        let spawn_input = arguments.clone();
        let input = validate_spawn_arguments(arguments)?;
        let span = tracing::Span::current();
        span.record("provider", tracing::field::display(input.provider.as_str()));
        span.record("mode", tracing::field::display(input.mode.as_str()));
        let limit = max_active_tasks();
        if self.active.len() >= limit {
            return Err(format!(
                "too many active tasks: {} of {} slots in use. Wait for a task to finish or stop one (agent_stop) before spawning. Raise the ceiling with AGENT_BRIDGE_MAX_ACTIVE_TASKS.",
                self.active.len(),
                limit
            ));
        }
        let agent_id = self.next_agent_id();
        span.record("agent_id", tracing::field::display(&agent_id));
        let watch_sender = self.ensure_watch_sender(&agent_id).clone();
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
            retry_policy: input.retry_policy.clone(),
            attempt_count: 0,
            parent_agent_id: None,
            spawn_input,
            partial_results: Vec::new(),
        };

        let outcome = launch_task(
            agent_id.clone(),
            input.mode,
            command,
            agent_dir,
            self.tx.clone(),
            watch_sender,
        )
        .await;
        if let Some(active) = apply_launch_outcome(&mut record, outcome, &cwd).await? {
            self.active.insert(agent_id.clone(), active);
        }
        self.registry.tasks.insert(agent_id.clone(), record);
        self.save().await?;
        // Spawn is a one-shot launch (not a polling loop), so include launch detail
        // (pid, isolation, worktreePath, profile) for the caller.
        let task = self.registry.tasks.get(&agent_id).unwrap();
        let mut value = public_task(task);
        add_detail(&mut value, task);
        Ok(value)
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
        self.signal_task(agent_id);
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
        // Remove the agent directory before persisting the terminal state so a
        // failure is recorded on the task record (and remains discoverable in the
        // on-disk registry for the reconciliation sweep) rather than silently
        // orphaning the directory.
        let dir_removal = fs::remove_dir_all(&agent_dir).await;
        if let Err(error) = &dir_removal {
            task.diagnostic = Some(json!({
                "failureCategory": FailureCategory::AgentDirCleanupFailed.as_str(),
                "message": format!("failed to remove agent directory: {error}"),
                "agentDir": agent_dir,
            }));
        }
        transition_status(task, TaskStatus::Removed)?;
        task.updated_at = now_iso();
        self.save().await?;
        self.watches.remove(agent_id);
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

    #[tracing::instrument(
        name = "finalize_task",
        skip(self, completion),
        fields(
            agent_id = %completion.agent_id,
            exit_code = ?completion.exit_code,
            signal = ?completion.signal,
            error_type = ?completion.error_type,
            duration_ms = tracing::field::Empty,
            task_status = ?completion.status
        )
    )]
    async fn complete(&mut self, completion: TaskCompletion) -> Result<(), String> {
        let finalize_started = Instant::now();
        self.active.remove(&completion.agent_id);

        let retry_info = {
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
            if partial_result_detected && !final_result_detected {
                task.partial_results = crate::task::complete::scan_partial_results(&task.agent_dir);
            }
            task.transcript_diagnostic = if transcript_available {
                None
            } else {
                Some(json!({
                    "failureCategory": FailureCategory::TranscriptUnavailable.as_str(),
                    "message": "transcript artifact was not available during finalization"
                }))
            };
            let result_path = PathBuf::from(&task.agent_dir).join("result.json");
            let result_json = serde_json::to_vec_pretty(task).map_err(|error| error.to_string())?;
            fs::write(result_path, result_json)
                .await
                .map_err(|error| error.to_string())?;

            let mut info = None;
            if let Some(policy) = &task.retry_policy
                && task.attempt_count < policy.max_retries
            {
                let category = task
                    .diagnostic
                    .as_ref()
                    .and_then(|d| d.get("failureCategory"))
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<FailureCategory>().ok());
                if let Some(category) = category
                    && category.is_transient()
                {
                    let next_attempt = task.attempt_count + 1;
                    let backoff = compute_jittered_backoff(next_attempt, policy.backoff_ms);
                    task.attempt_count = next_attempt;
                    let origin_agent_id = task
                        .parent_agent_id
                        .clone()
                        .unwrap_or_else(|| task.agent_id.clone());
                    info = Some((
                        backoff,
                        task.spawn_input.clone(),
                        origin_agent_id,
                        task.agent_dir.clone(),
                        task.retry_policy.clone(),
                        next_attempt,
                        task.provider,
                    ));
                }
            }
            info
        };

        self.save().await?;
        self.signal_task(&completion.agent_id);
        tracing::Span::current().record(
            "duration_ms",
            tracing::field::display(finalize_started.elapsed().as_millis()),
        );

        if let Some((
            backoff,
            spawn_input,
            parent_agent_id,
            parent_agent_dir,
            retry_policy,
            next_attempt,
            provider,
        )) = retry_info
        {
            let _ = append_transcript_event(
                &PathBuf::from(&parent_agent_dir).join("transcript.jsonl"),
                provider,
                "lifecycle",
                "lifecycle",
                "",
                json!({
                    "phase": "retry_attempt",
                    "attemptCount": next_attempt,
                    "maxRetries": retry_policy.as_ref().unwrap().max_retries,
                    "backoffMs": backoff,
                    "parentAgentId": parent_agent_id,
                }),
                &provider_env_redactions(provider),
            )
            .await;

            sleep(Duration::from_millis(backoff)).await;

            let mut args = spawn_input;
            if let Some(obj) = args.as_object_mut() {
                obj.remove("dryRun");
            }

            match self.spawn(args).await {
                Ok(public) => {
                    if let Some(new_id) = public["agentId"].as_str() {
                        if let Some(new_task) = self.registry.tasks.get_mut(new_id) {
                            new_task.attempt_count = next_attempt;
                            new_task.retry_policy = retry_policy;
                            new_task.parent_agent_id = Some(parent_agent_id);
                        }
                        let _ = self.save().await;
                    }
                }
                Err(error) => {
                    tracing::error!(
                        parent_agent_id = %parent_agent_id,
                        error = %error,
                        "[agent-bridge] retry spawn failed for {parent_agent_id}: {error}"
                    );
                }
            }
        }

        Ok(())
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

    fn subscribe_task(&mut self, agent_id: &str) -> Result<watch::Receiver<u64>, String> {
        self.require_task(agent_id)?;
        Ok(self.ensure_watch_sender(agent_id).subscribe())
    }

    fn ensure_watch_sender(&mut self, agent_id: &str) -> &watch::Sender<u64> {
        self.watches
            .entry(agent_id.to_string())
            .or_insert_with(|| watch::channel(0).0)
    }

    fn signal_task(&mut self, agent_id: &str) {
        if let Some(sender) = self.watches.get(agent_id) {
            sender.send_modify(|version| *version = version.wrapping_add(1));
        }
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

/// Computes jittered exponential backoff for a retry attempt.
/// Base doubles each attempt (starting at attempt 1). Clamp to [1000, 30000] ms.
/// Jitter is +/- 25% drawn from wall-clock nanosecond entropy.
fn compute_jittered_backoff(attempt_count: u32, base_backoff_ms: u64) -> u64 {
    let base = base_backoff_ms.max(1000);
    let exp = attempt_count.saturating_sub(1).min(6);
    let raw = base.saturating_mul(1u64 << exp);
    let clamped = raw.clamp(1000, 30000);
    let jitter_seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let jitter_range = (clamped / 4).max(1);
    let jitter_window = jitter_range * 2 + 1;
    let jitter_offset = (jitter_seed.wrapping_add(attempt_count as u64)) % jitter_window;
    let jittered = clamped
        .saturating_sub(jitter_range)
        .saturating_add(jitter_offset);
    jittered.clamp(1000, 30000)
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_policy: Option<RetryPolicy>,
    #[serde(default)]
    pub attempt_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_agent_id: Option<String>,
    #[serde(default)]
    pub spawn_input: Value,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub partial_results: Vec<PartialResult>,
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
#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "agent-bridge-mcp-{name}-{}",
            Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn sample_task(status: TaskStatus) -> TaskRecord {
        TaskRecord {
            agent_id: "agent_11111111111111111111111111111".to_string(),
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
            retry_policy: None,
            attempt_count: 0,
            parent_agent_id: None,
            spawn_input: Value::Null,
            partial_results: Vec::new(),
        }
    }

    fn next_items(task: &Value) -> &Vec<Value> {
        task["next"].as_array().unwrap()
    }

    fn next_action(task: &Value, index: usize) -> &Value {
        &next_items(task)[index]
    }

    fn next_ids(task: &Value) -> Vec<String> {
        next_items(task)
            .iter()
            .map(|item| item["id"].as_str().unwrap().to_string())
            .collect()
    }

    fn next_item<'a>(task: &'a Value, id: &str) -> &'a Value {
        next_items(task)
            .iter()
            .find(|item| item["id"] == id)
            .unwrap_or_else(|| panic!("missing next action: {id}"))
    }

    #[test]
    fn insert_outcome_fields_writes_exit_signal_error_and_type() {
        let mut task = sample_task(TaskStatus::Failed);
        task.exit_code = Some(2);
        task.signal = Some("SIGKILL".to_string());
        task.error = Some("boom".to_string());
        task.error_type = Some(ErrorType::Stale);

        let mut value = json!({});
        insert_outcome_fields(&mut value, &task);

        assert_eq!(value["exitCode"], json!(2));
        assert_eq!(value["signal"], json!("SIGKILL"));
        assert_eq!(value["error"], json!("boom"));
        assert_eq!(value["errorType"], json!(task.error_type));
    }

    #[test]
    fn insert_outcome_fields_writes_nulls_when_absent() {
        let task = sample_task(TaskStatus::Running);
        let mut value = json!({});
        insert_outcome_fields(&mut value, &task);

        assert_eq!(value["exitCode"], Value::Null);
        assert_eq!(value["signal"], Value::Null);
        assert_eq!(value["error"], Value::Null);
    }

    #[test]
    fn insert_evidence_fields_respects_section_flags() {
        let mut task = sample_task(TaskStatus::Succeeded);
        task.changed_files = Some(vec!["README.md".to_string()]);
        task.git_status = Some(" M README.md".to_string());
        task.git_diff = Some("diff --git".to_string());

        // changed_files + diff requested, reviewPacket always present.
        let mut value = json!({});
        let sections = ResultSections {
            changed_files: true,
            stdout: false,
            stderr: false,
            diff: true,
            transcript: false,
        };
        insert_evidence_fields(&mut value, &task, &sections, false, false);
        assert!(value["reviewPacket"].is_object());
        assert_eq!(value["changedFiles"], json!(["README.md"]));
        assert_eq!(value["gitStatus"], json!(" M README.md"));
        assert_eq!(value["gitDiff"], json!("diff --git"));

        // neither changed_files nor diff requested -> only reviewPacket.
        let mut bare = json!({});
        insert_evidence_fields(
            &mut bare,
            &task,
            &ResultSections::default_sections(),
            false,
            false,
        );
        assert!(bare["reviewPacket"].is_object());
        // default_sections has changed_files=true, so assert diff is omitted instead.
        assert!(bare.get("gitDiff").is_none());
    }

    #[test]
    fn insert_detail_fields_writes_detail_flags() {
        let mut task = sample_task(TaskStatus::Succeeded);
        task.transcript_available = true;
        task.final_result_detected = true;
        task.partial_result_detected = false;

        let mut value = json!({});
        insert_detail_fields(&mut value, &task);

        assert_eq!(value["transcriptAvailable"], json!(true));
        assert_eq!(value["finalResultDetected"], json!(true));
        assert_eq!(value["partialResultDetected"], json!(false));
        assert!(value.as_object().unwrap().contains_key("diagnostic"));
    }

    #[test]
    fn transition_status_rejects_illegal_moves() {
        let mut task = sample_task(TaskStatus::Queued);
        transition_status(&mut task, TaskStatus::Running).unwrap();
        transition_status(&mut task, TaskStatus::Succeeded).unwrap();

        assert!(transition_status(&mut task, TaskStatus::Running).is_err());
    }

    #[test]
    fn public_agent_returns_lean_envelope_for_all_lifecycle_states() {
        for (status, phase) in [
            (TaskStatus::Queued, "pending"),
            (TaskStatus::Running, "active"),
            (TaskStatus::Succeeded, "done"),
            (TaskStatus::Failed, "done"),
            (TaskStatus::Stopped, "done"),
            (TaskStatus::FailedStale, "done"),
        ] {
            let mut task = sample_task(status);
            task.title = Some("Presentation audit".to_string());
            task.transcript_available = true;
            task.changed_files = Some(vec!["README.md".to_string()]);
            if status == TaskStatus::FailedStale {
                task.error_type = Some(ErrorType::Stale);
            }

            let public = public_task(&task);

            // Lean envelope: each field once, no GUI presentation chrome.
            assert_eq!(public["agentId"], task.agent_id);
            assert_eq!(public["status"], json!(status));
            assert_eq!(public["phase"], phase);
            assert_eq!(public["isFinal"], is_final(status));
            assert!(public["progress"].is_object());
            assert!(public["next"].is_array());
            assert!(public.get("presentation").is_none());
            assert!(public.get("nextActions").is_none());
            assert!(public.get("stdout").is_none());
            assert!(public.get("gitDiff").is_none());
        }
    }

    #[test]
    fn display_title_uses_safe_provider_mode_fallback() {
        let mut task = sample_task(TaskStatus::Running);
        task.title = None;
        assert_eq!(display_title(&task), "codex review task");
    }

    #[test]
    fn next_actions_reflect_running_final_and_worktree_states() {
        let mut running = sample_task(TaskStatus::Running);
        running.transcript_available = true;
        let running_public = public_task(&running);
        // Lean envelope: a single `next` list, no GUI `presentation`/`actions`.
        assert!(running_public.get("presentation").is_none());
        let running_ids = next_ids(&running_public);
        assert_eq!(next_action(&running_public, 0)["id"], "observe");
        assert_eq!(next_action(&running_public, 0)["tool"], "agent_observe");
        assert_eq!(
            next_action(&running_public, 0)["arguments"]["agentId"],
            running.agent_id
        );
        assert_eq!(next_action(&running_public, 0)["safety"], "safe");
        assert!(running_ids.contains(&"wait_final".to_string()));
        assert!(running_ids.contains(&"stop".to_string()));
        assert!(!running_ids.contains(&"inspect_result".to_string()));
        // wait_final subsumes the former agent_wait via agent_observe until:"final".
        let wait = next_item(&running_public, "wait_final");
        assert_eq!(wait["tool"], "agent_observe");
        assert_eq!(wait["arguments"]["until"], "final");

        let mut final_task = sample_task(TaskStatus::Succeeded);
        final_task.transcript_available = false;
        let final_public = public_task(&final_task);
        assert_eq!(next_action(&final_public, 0)["id"], "inspect_result");
        assert_eq!(next_action(&final_public, 0)["tool"], "agent_result");
        assert!(!next_ids(&final_public).contains(&"cleanup".to_string()));

        let mut worktree = sample_task(TaskStatus::Succeeded);
        worktree.worktree_managed = true;
        worktree.worktree_path = Some("/tmp/worktree".to_string());
        let worktree_public = public_task(&worktree);
        assert_eq!(next_action(&worktree_public, 0)["id"], "inspect_result");
        assert_eq!(next_action(&worktree_public, 1)["id"], "cleanup");
        assert_eq!(next_action(&worktree_public, 1)["state"], "unsafe");

        worktree.result_inspected_at = Some(now_iso());
        let inspected_worktree_public = public_task(&worktree);
        // Succeeded + inspected + no error => verify_project is primary, cleanup destructive.
        assert_eq!(
            next_action(&inspected_worktree_public, 0)["id"],
            "verify_project"
        );
        assert_eq!(
            next_item(&inspected_worktree_public, "cleanup")["safety"],
            "destructive"
        );

        let mut stale = sample_task(TaskStatus::FailedStale);
        stale.error_type = Some(ErrorType::Stale);
        stale.result_inspected_at = Some(now_iso());
        let stale_public = public_task(&stale);
        assert_eq!(stale_public["phase"], "done");
        assert_eq!(next_action(&stale_public, 0)["id"], "inspect_evidence");
        assert_eq!(next_action(&stale_public, 0)["tool"], "agent_result");
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
        // Lean per-agent summaries: a single `next` list, no GUI presentation blob.
        assert!(tasks.iter().all(|task| task["next"].is_array()));
        assert!(tasks.iter().all(|task| task.get("presentation").is_none()));
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

    // Supervision registry/signal tests live in `task::supervision`.

    #[test]
    fn max_active_tasks_defaults_when_unset() {
        // Exercises the default branch; env-driven overrides are validated by the
        // parsing logic (positive integers only).
        assert!(max_active_tasks() >= 1);
    }

    async fn make_actor_with_task(task: TaskRecord) -> (TaskActor, mpsc::Receiver<ActorCommand>) {
        let state_dir = temp_dir("actor-remove");
        let mut registry = Registry {
            tasks: BTreeMap::new(),
        };
        registry.tasks.insert(task.agent_id.clone(), task);
        let (tx, rx) = mpsc::channel(16);
        let actor = TaskActor {
            state_dir,
            registry,
            active: BTreeMap::new(),
            watches: BTreeMap::new(),
            tx,
        };
        (actor, rx)
    }

    #[tokio::test]
    async fn observe_wakes_promptly_when_task_watch_is_signaled() {
        let agent_dir = temp_dir("observe-watch-agent");
        let mut task = sample_task(TaskStatus::Running);
        task.agent_dir = agent_dir.display().to_string();
        let agent_id = task.agent_id.clone();
        let transcript_path = agent_dir.join("transcript.jsonl");
        let state_dir = temp_dir("observe-watch-state");
        let mut registry = Registry {
            tasks: BTreeMap::new(),
        };
        registry.tasks.insert(agent_id.clone(), task);
        let (tx, rx) = mpsc::channel(16);
        let mut actor = TaskActor {
            state_dir,
            registry,
            active: BTreeMap::new(),
            watches: BTreeMap::new(),
            tx: tx.clone(),
        };
        let watch = actor.ensure_watch_sender(&agent_id).clone();
        tokio::spawn(actor.run(rx));
        let handle = TaskManagerHandle { tx };

        let started = Instant::now();
        let observe = tokio::spawn({
            let agent_id = agent_id.clone();
            async move {
                handle
                    .observe(agent_id, Some(0), Some(10), Some(5_000), false)
                    .await
                    .unwrap()
            }
        });
        tokio::spawn(async move {
            sleep(Duration::from_millis(10)).await;
            append_transcript_event(
                &transcript_path,
                ProviderKind::Codex,
                "stdout",
                "provider_result",
                "ready",
                json!({"result": "ready"}),
                &[],
            )
            .await;
            for _ in 0..20 {
                watch.send_modify(|version| *version = version.wrapping_add(1));
                sleep(Duration::from_millis(2)).await;
            }
        });

        let observed = tokio::time::timeout(Duration::from_millis(250), observe)
            .await
            .unwrap()
            .unwrap();
        assert!(
            started.elapsed() < Duration::from_millis(45),
            "observe should wake from watch signal before the old 50ms poll interval"
        );
        assert_eq!(observed["timedOut"], false);
        assert!(!observed["events"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn remove_returns_error_on_git_worktree_failure() {
        let mut task = sample_task(TaskStatus::Succeeded);
        task.worktree_managed = true;
        task.worktree_path = Some("/tmp/nonexistent-agent-bridge-worktree-test".to_string());
        task.original_cwd = Some(".".to_string());
        task.agent_dir = ".".to_string();
        let (mut actor, _rx) = make_actor_with_task(task).await;

        let error = actor.remove("agent_11111111111111111111111111111").await;
        assert!(error.is_err());
        let msg = error.unwrap_err();
        assert!(
            msg.contains("worktree") || msg.contains("git"),
            "expected worktree/git error, got: {msg}"
        );
    }

    #[tokio::test]
    async fn remove_records_diagnostic_on_dir_cleanup_failure() {
        let state_dir = temp_dir("remove-dir-failure");
        let mut task = sample_task(TaskStatus::Succeeded);
        // Point agent_dir at a read-only directory so remove_dir_all fails.
        // Use /dev/null as agent_dir — it exists but is not a directory,
        // so remove_dir_all will fail with ENOTDIR.
        task.agent_dir = "/dev/null".to_string();
        task.worktree_managed = false;
        let mut registry = Registry {
            tasks: BTreeMap::new(),
        };
        registry.tasks.insert(task.agent_id.clone(), task);
        let (tx, _rx) = mpsc::channel(16);
        let mut actor = TaskActor {
            state_dir,
            registry,
            active: BTreeMap::new(),
            watches: BTreeMap::new(),
            tx,
        };

        let result = actor.remove("agent_11111111111111111111111111111").await;
        assert!(
            result.is_ok(),
            "remove should succeed even with dir cleanup failure"
        );

        // Verify diagnostic was recorded on the task record.
        let task_record = actor
            .registry
            .tasks
            .get("agent_11111111111111111111111111111")
            .unwrap();
        let diag = task_record
            .diagnostic
            .as_ref()
            .expect("expected diagnostic");
        assert_eq!(diag["failureCategory"], "agent_dir_cleanup_failed");
    }

    #[tokio::test]
    async fn remove_returns_error_on_save_failure() {
        let state_dir = temp_dir("remove-save-failure");
        let mut task = sample_task(TaskStatus::Succeeded);
        task.agent_dir = state_dir.join("agent").display().to_string();
        std::fs::create_dir_all(&task.agent_dir).unwrap();
        task.worktree_managed = false;
        let mut registry = Registry {
            tasks: BTreeMap::new(),
        };
        registry.tasks.insert(task.agent_id.clone(), task);
        let (tx, _rx) = mpsc::channel(16);
        let mut actor = TaskActor {
            state_dir: state_dir.clone(),
            registry,
            active: BTreeMap::new(),
            watches: BTreeMap::new(),
            tx,
        };

        // Write an initial registry so save can attempt an atomic rename.
        save_registry(&state_dir, &actor.registry).await.unwrap();

        // Replace the state dir with a file so save_registry's create_dir_all fails.
        std::fs::remove_dir_all(&state_dir).unwrap();
        std::fs::write(&state_dir, b"blocking-file").unwrap();

        let result = actor.remove("agent_11111111111111111111111111111").await;
        // Clean up the blocking file.
        std::fs::remove_file(&state_dir).ok();

        assert!(
            result.is_err(),
            "remove should fail when registry save fails"
        );
        let msg = result.unwrap_err();
        assert!(!msg.is_empty(), "expected error");
    }

    #[tokio::test]
    async fn retry_exhaustion_schedules_respawns_within_budget() {
        let state_dir = temp_dir("retry-budget");
        let mut task = sample_task(TaskStatus::Running);
        task.agent_id = "agent_original".to_string();
        task.agent_dir = state_dir
            .join("tasks")
            .join("agent_original")
            .display()
            .to_string();
        std::fs::create_dir_all(&task.agent_dir).unwrap();
        task.retry_policy = Some(RetryPolicy {
            max_retries: 2,
            backoff_ms: 1000,
        });
        task.spawn_input = json!({
            "provider": "kimi",
            "mode": "research",
            "prompt": "test prompt",
            "cwd": ".",
            "timeoutSeconds": 1
        });
        task.attempt_count = 0;

        let mut registry = Registry {
            tasks: BTreeMap::new(),
        };
        registry.tasks.insert(task.agent_id.clone(), task);
        let (tx, _rx) = mpsc::channel(16);
        let mut actor = TaskActor {
            state_dir: state_dir.clone(),
            registry,
            active: BTreeMap::new(),
            watches: BTreeMap::new(),
            tx,
        };

        let transient_completion = |agent_id: &str| TaskCompletion {
            agent_id: agent_id.to_string(),
            status: TaskStatus::Failed,
            exit_code: None,
            signal: None,
            error: Some("timeout".to_string()),
            error_type: Some(ErrorType::Timeout),
            diagnostic: Some(json!({
                "failureCategory": FailureCategory::ProviderTimeout.as_str(),
            })),
        };

        // First failure triggers a retry.
        let start = Instant::now();
        actor
            .complete(transient_completion("agent_original"))
            .await
            .unwrap();
        let elapsed_first = start.elapsed();
        assert!(
            actor.registry.tasks.len() >= 2,
            "expected retry task to be created"
        );

        let retry_id_1 = actor
            .registry
            .tasks
            .keys()
            .find(|id| *id != "agent_original")
            .cloned()
            .expect("retry task should exist");
        let retry_task_1 = actor.registry.tasks.get(&retry_id_1).unwrap();
        assert_eq!(
            retry_task_1.parent_agent_id,
            Some("agent_original".to_string())
        );
        assert_eq!(retry_task_1.attempt_count, 1);

        // Second failure triggers another retry.
        actor
            .complete(transient_completion(&retry_id_1))
            .await
            .unwrap();
        assert!(
            actor.registry.tasks.len() >= 3,
            "expected second retry task"
        );

        let retry_id_2 = actor
            .registry
            .tasks
            .keys()
            .find(|id| *id != "agent_original" && *id != &retry_id_1)
            .cloned()
            .expect("second retry task should exist");
        let retry_task_2 = actor.registry.tasks.get(&retry_id_2).unwrap();
        assert_eq!(
            retry_task_2.parent_agent_id,
            Some("agent_original".to_string())
        );
        assert_eq!(retry_task_2.attempt_count, 2);

        // Third failure exhausts the budget.
        actor
            .complete(transient_completion(&retry_id_2))
            .await
            .unwrap();
        assert_eq!(
            actor.registry.tasks.len(),
            3,
            "should not create more retries after exhaustion"
        );

        // Effective backoff is at least 750 ms (1000 ms minus 25% jitter).
        assert!(
            elapsed_first >= Duration::from_millis(750),
            "expected backoff delay before first retry, got {:?}",
            elapsed_first
        );
    }

    #[tokio::test]
    async fn permanent_failure_does_not_trigger_retry() {
        let state_dir = temp_dir("retry-permanent");
        let mut task = sample_task(TaskStatus::Running);
        task.agent_id = "agent_perm".to_string();
        task.agent_dir = state_dir
            .join("tasks")
            .join("agent_perm")
            .display()
            .to_string();
        std::fs::create_dir_all(&task.agent_dir).unwrap();
        task.retry_policy = Some(RetryPolicy {
            max_retries: 2,
            backoff_ms: 1000,
        });
        task.spawn_input = json!({
            "provider": "kimi",
            "mode": "research",
            "prompt": "test prompt",
            "cwd": ".",
            "timeoutSeconds": 1
        });
        task.attempt_count = 0;

        let mut registry = Registry {
            tasks: BTreeMap::new(),
        };
        registry.tasks.insert(task.agent_id.clone(), task);
        let (tx, _rx) = mpsc::channel(16);
        let mut actor = TaskActor {
            state_dir,
            registry,
            active: BTreeMap::new(),
            watches: BTreeMap::new(),
            tx,
        };

        let permanent_completion = TaskCompletion {
            agent_id: "agent_perm".to_string(),
            status: TaskStatus::Failed,
            exit_code: Some(1),
            signal: None,
            error: Some("exit error".to_string()),
            error_type: Some(ErrorType::ProviderExitError),
            diagnostic: Some(json!({
                "failureCategory": FailureCategory::ProviderExitError.as_str(),
            })),
        };

        actor.complete(permanent_completion).await.unwrap();
        assert_eq!(
            actor.registry.tasks.len(),
            1,
            "permanent failure should not trigger retry"
        );
        let task_record = actor.registry.tasks.get("agent_perm").unwrap();
        assert_eq!(task_record.attempt_count, 0);
    }

    #[tokio::test]
    async fn retry_appends_transcript_event_before_respawn() {
        let state_dir = temp_dir("retry-transcript");
        let mut task = sample_task(TaskStatus::Running);
        task.agent_id = "agent_trans".to_string();
        let agent_dir = state_dir.join("tasks").join("agent_trans");
        task.agent_dir = agent_dir.display().to_string();
        std::fs::create_dir_all(&agent_dir).unwrap();
        task.retry_policy = Some(RetryPolicy {
            max_retries: 1,
            backoff_ms: 1000,
        });
        task.spawn_input = json!({
            "provider": "kimi",
            "mode": "research",
            "prompt": "test prompt",
            "cwd": ".",
            "timeoutSeconds": 1
        });
        task.attempt_count = 0;

        let mut registry = Registry {
            tasks: BTreeMap::new(),
        };
        registry.tasks.insert(task.agent_id.clone(), task);
        let (tx, _rx) = mpsc::channel(16);
        let mut actor = TaskActor {
            state_dir,
            registry,
            active: BTreeMap::new(),
            watches: BTreeMap::new(),
            tx,
        };

        let completion = TaskCompletion {
            agent_id: "agent_trans".to_string(),
            status: TaskStatus::Failed,
            exit_code: None,
            signal: None,
            error: Some("timeout".to_string()),
            error_type: Some(ErrorType::Timeout),
            diagnostic: Some(json!({
                "failureCategory": FailureCategory::ProviderTimeout.as_str(),
            })),
        };

        actor.complete(completion).await.unwrap();

        let transcript_path = agent_dir.join("transcript.jsonl");
        let transcript = tokio::fs::read_to_string(&transcript_path).await.unwrap();
        assert!(
            transcript.contains("retry_attempt"),
            "transcript should contain retry_attempt event: {transcript}"
        );
    }

    #[test]
    fn compute_jittered_backoff_clamps_and_jitters() {
        // Attempt 1: base doubled 0 times, so base itself.
        let b1 = compute_jittered_backoff(1, 1000);
        assert!(
            (1000..=1250).contains(&b1),
            "attempt 1 backoff out of range: {b1}"
        );

        // Attempt 4: base * 8 = 8000, still within clamp.
        let b4 = compute_jittered_backoff(4, 1000);
        assert!(
            (6000..=10000).contains(&b4),
            "attempt 4 backoff out of range: {b4}"
        );

        // Large base should clamp at 30000.
        let b_cap = compute_jittered_backoff(1, 60000);
        assert!(
            b_cap <= 30000,
            "capped backoff exceeded max+jitter: {b_cap}"
        );
        assert!(b_cap >= 22500, "capped backoff below min+jitter: {b_cap}");

        // Small base should floor at 1000.
        let b_floor = compute_jittered_backoff(1, 100);
        assert!(
            (1000..=1250).contains(&b_floor),
            "floored backoff out of range: {b_floor}"
        );
    }
}
