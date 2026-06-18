use crate::domain::{FailureCategory, ProviderKind, TaskMode};
use crate::mcp::{JsonRpcId, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use crate::router::{
    AttemptDisposition, AttemptEvidence, RoutedAttemptInput, RouterPolicy, RouterStopReason,
    classify_attempt,
};
use crate::server::handle_request;
use crate::task::TaskManagerHandle;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use uuid::Uuid;

const ROUTER_EVIDENCE_UPDATE_LIMIT: usize = 20;

#[derive(Parser, Debug)]
#[command(
    name = "agent-bridge-mcp",
    version,
    about = "Rust stdio MCP server for delegating bounded tasks to local provider agents",
    after_help = "Config file: ~/.agent-bridge-mcp/config.toml\nLegacy env vars still work: AGENT_BRIDGE_WORKSPACES, AGENT_BRIDGE_STATE_DIR, AGENT_BRIDGE_CLAUDE_HOST_SOCKET, AGENT_BRIDGE_MAX_ACTIVE_TASKS"
)]
struct Cli {
    #[command(flatten)]
    config: crate::config::ConfigCliOverrides,
    /// Validate layered config and print a JSON summary without starting MCP stdio.
    #[arg(long, conflicts_with = "doctor_smoke")]
    config_check: bool,
    /// Run provider readiness smoke and print the doctor provider JSON report.
    #[arg(long)]
    doctor_smoke: bool,
    /// Restrict --doctor-smoke to one provider. Repeat for multiple providers.
    #[arg(long = "provider", value_name = "PROVIDER")]
    providers: Vec<crate::domain::ProviderKind>,
    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(Subcommand, Debug)]
enum CliCommand {
    /// Ask a running Agent Bridge server to reload config from disk.
    Reload,
    /// Run the ACP router runtime over newline-delimited JSON-RPC.
    AcpRouter,
    /// Run the Agent Bridge-owned Claude host runner on the given Unix socket.
    ClaudeHostRunner { socket: std::path::PathBuf },
}

pub async fn main_entry() {
    init_tracing();
    install_panic_hook();
    if std::env::var("AGENT_BRIDGE_FORCE_PANIC").ok().as_deref() == Some("1") {
        panic!("forced panic for integration test");
    }
    let cli = Cli::parse();
    if cli.config_check {
        exit_with_config_check(cli.config);
    }
    if cli.doctor_smoke {
        exit_with_doctor_smoke(cli.providers).await;
    }
    match cli.command {
        Some(CliCommand::Reload) => exit_with_reload(cli.config),
        Some(CliCommand::AcpRouter) => {
            if let Err(error) = run_acp_router().await {
                tracing::error!(error = %error, "[agent-bridge] fatal {error}");
                std::process::exit(1);
            }
            return;
        }
        Some(CliCommand::ClaudeHostRunner {
            socket: socket_path,
        }) => {
            if let Err(error) = crate::claude_host::run_server(socket_path).await {
                tracing::error!(error = %error, "[agent-bridge] fatal {error}");
                std::process::exit(1);
            }
            return;
        }
        None => {}
    }

    let state_dir = match crate::config::Config::from_env(cli.config.clone()) {
        Ok(config) => {
            if let Err(error) = crate::config::install_runtime_config(&config) {
                tracing::error!(error = %error, "[agent-bridge] fatal {error}");
                std::process::exit(1);
            }
            config.state_dir().to_path_buf()
        }
        Err(error) => {
            tracing::error!(
                error = %error,
                "[agent-bridge] startup config load failed; starting with state-dir-only fallback so doctor can report diagnostics"
            );
            crate::config::reload_pid_state_dir(&cli.config)
        }
    };
    let pid_lock = match PidLock::acquire(&state_dir) {
        Ok(lock) => lock,
        Err(error) => {
            tracing::error!(error = %error, "[agent-bridge] fatal {error}");
            std::process::exit(1);
        }
    };
    install_reload_handler(cli.config);
    match run_until_shutdown().await {
        Ok(exit_code) => {
            drop(pid_lock);
            if exit_code != 0 {
                std::process::exit(exit_code);
            }
        }
        Err(error) => {
            drop(pid_lock);
            tracing::error!(error = %error, "[agent-bridge] fatal {error}");
            std::process::exit(1);
        }
    }
}

async fn run_acp_router() -> io::Result<()> {
    let stdin = io::stdin();
    let mut lines = BufReader::new(stdin).lines();
    let mut stdout = io::stdout();
    let mut sessions = HashMap::new();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let request: Result<JsonRpcRequest, _> = serde_json::from_str(&line);
        match request {
            Ok(request) => {
                if let Some(response) =
                    handle_acp_router_request(request, &mut sessions, &mut stdout).await?
                {
                    write_response(&mut stdout, &response).await?;
                }
            }
            Err(_) => {
                let response = JsonRpcResponse::error(JsonRpcId::Null, -32700, "Parse error");
                write_response(&mut stdout, &response).await?;
            }
        }
    }
    Ok(())
}

struct RouterSession {
    cwd: Option<String>,
}

async fn handle_acp_router_request(
    request: JsonRpcRequest,
    sessions: &mut HashMap<String, RouterSession>,
    stdout: &mut io::Stdout,
) -> io::Result<Option<JsonRpcResponse>> {
    let Some(id) = request.id else {
        return Ok(None);
    };
    let response = match request.method.as_str() {
        "initialize" => JsonRpcResponse::result(
            id,
            json!({
                "protocolVersion": 1,
                "agentCapabilities": {},
                "sessionCapabilities": {}
            }),
        ),
        "session/new" => {
            let params = parse_acp_params::<AcpNewSessionParams>(request.params);
            let session_id = format!("router-{}", Uuid::new_v4().simple());
            sessions.insert(session_id.clone(), RouterSession { cwd: params.cwd });
            JsonRpcResponse::result(id, json!({"sessionId": session_id}))
        }
        "session/prompt" => {
            return run_acp_router_prompt(id, request.params, sessions, stdout).await;
        }
        _ => JsonRpcResponse::error(
            id,
            -32601,
            "method not supported by Agent Bridge ACP router",
        ),
    };
    Ok(Some(response))
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AcpNewSessionParams {
    cwd: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AcpPromptParams {
    session_id: String,
    prompt: Value,
    policy: Option<AcpPromptPolicy>,
    mode: Option<TaskMode>,
    timeout_seconds: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AcpPromptPolicy {
    candidates: Vec<ProviderKind>,
}

fn parse_acp_params<T>(params: Option<Value>) -> T
where
    T: Default + for<'de> Deserialize<'de>,
{
    params
        .map(serde_json::from_value)
        .transpose()
        .ok()
        .flatten()
        .unwrap_or_default()
}

async fn run_acp_router_prompt(
    id: JsonRpcId,
    params: Option<Value>,
    sessions: &HashMap<String, RouterSession>,
    stdout: &mut io::Stdout,
) -> io::Result<Option<JsonRpcResponse>> {
    let params = match params.map(serde_json::from_value::<AcpPromptParams>) {
        Some(Ok(params)) => params,
        _ => {
            return Ok(Some(JsonRpcResponse::error(
                id,
                -32602,
                "invalid session/prompt params",
            )));
        }
    };
    let Some(session) = sessions.get(&params.session_id) else {
        return Ok(Some(JsonRpcResponse::error(
            id,
            -32602,
            "unknown router sessionId",
        )));
    };
    let Some(prompt) = acp_prompt_text(&params.prompt) else {
        return Ok(Some(JsonRpcResponse::error(
            id,
            -32602,
            "session/prompt requires text prompt content",
        )));
    };
    let candidates = params
        .policy
        .map(|policy| policy.candidates)
        .unwrap_or_else(|| vec![ProviderKind::Codex, ProviderKind::Claude]);
    let policy = match RouterPolicy::new(candidates) {
        Ok(policy) => policy,
        Err(error) => return Ok(Some(JsonRpcResponse::error(id, -32602, error.to_string()))),
    };
    if policy.candidates.is_empty() {
        return Ok(Some(JsonRpcResponse::error(
            id,
            -32602,
            "router policy requires at least one candidate",
        )));
    }
    let manager = match TaskManagerHandle::from_env().await {
        Ok(manager) => manager,
        Err(error) => return Ok(Some(JsonRpcResponse::error(id, -32000, error))),
    };
    let mode = params.mode.unwrap_or(TaskMode::Implement);
    let timeout_ms = params
        .timeout_seconds
        .map(|seconds| seconds.saturating_mul(1_000).saturating_add(1_000));
    let mut attempts = Vec::new();
    let mut evidence_refs = Vec::new();
    let mut failover_trail = Vec::new();
    let mut pending_failover: Option<Value> = None;
    let candidate_count = policy.candidates.len();
    for (index, provider) in policy.candidates.iter().copied().enumerate() {
        let execution = match manager
            .run_router_attempt(
                RoutedAttemptInput {
                    provider,
                    mode,
                    prompt: prompt.clone(),
                    title: Some("ACP router turn".to_string()),
                    cwd: session.cwd.clone(),
                    timeout_seconds: params.timeout_seconds,
                    isolation: None,
                    worktree_name: None,
                    profile: None,
                },
                timeout_ms,
            )
            .await
        {
            Ok(execution) => execution,
            Err(error) => return Ok(Some(JsonRpcResponse::error(id, -32000, error))),
        };
        let transcript = manager
            .transcript(execution.agent_id.clone(), None, Some(500))
            .await
            .ok();
        if let Some(transcript) = transcript.as_ref() {
            write_router_evidence_updates(stdout, &params.session_id, provider, transcript).await?;
        }
        let final_text = transcript.as_ref().and_then(transcript_final_text);
        let failure_category = router_failure_category(&execution.result)
            .or_else(|| router_wait_failure_category(&execution.wait_status));
        let stop_reason = router_stop_reason(&execution.result);
        let evidence = AttemptEvidence {
            final_text_present: final_text.is_some(),
            failure_category,
            stop_reason,
        };
        let disposition = classify_attempt(&evidence);
        let response_stop_reason =
            router_stop_reason_text(stop_reason.unwrap_or(RouterStopReason::EndTurn));
        let agent_id = execution.agent_id.clone();
        let evidence_ref = json!({
            "agentId": execution.evidence_ref.agent_id,
            "resultSections": execution.evidence_ref.result_sections,
            "transcriptAvailable": execution.evidence_ref.transcript_available
        });
        let attempt = json!({
            "provider": provider,
            "agentId": agent_id,
            "disposition": disposition,
            "stopReason": response_stop_reason,
            "failureCategory": failure_category,
            "evidenceRef": evidence_ref
        });
        if let Some(mut trail_entry) = pending_failover.take() {
            if let Some(object) = trail_entry.as_object_mut() {
                object.insert("targetProvider".to_string(), json!(provider));
                object.insert("targetAgentId".to_string(), json!(agent_id));
            }
            failover_trail.push(trail_entry);
        }
        attempts.push(attempt.clone());
        evidence_refs.push(evidence_ref.clone());
        if disposition == AttemptDisposition::FailoverEligible && index + 1 < candidate_count {
            pending_failover = Some(json!({
                "sourceProvider": provider,
                "sourceAgentId": agent_id,
                "failureCategory": failure_category,
                "reason": "failover_eligible"
            }));
            continue;
        }
        let routed_final_text = (disposition == AttemptDisposition::TrustedFinal)
            .then(|| final_text.clone())
            .flatten();
        if let Some(text) = routed_final_text.as_deref() {
            let notification = JsonRpcNotification::new(
                "session/update",
                json!({
                    "sessionId": params.session_id,
                    "update": {
                        "sessionUpdate": "agent_message_chunk",
                        "content": {"type": "text", "text": text}
                    }
                }),
            );
            write_json_message(stdout, &notification).await?;
        }
        let terminal_kind = router_terminal_kind(disposition);
        return Ok(Some(JsonRpcResponse::result(
            id,
            json!({
                "stopReason": response_stop_reason,
                "routerResult": {
                    "provider": provider,
                    "terminalKind": terminal_kind,
                    "finalText": routed_final_text,
                    "failureCategory": failure_category,
                    "blockerReason": router_blocker_reason(disposition, stop_reason, failure_category),
                    "attempts": attempts.clone(),
                    "diagnostics": {
                        "provider": provider,
                        "terminalKind": terminal_kind,
                        "attempts": attempts,
                        "failoverTrail": failover_trail,
                        "evidenceRefs": evidence_refs,
                        "bounded": true
                    }
                }
            }),
        )));
    }
    Ok(Some(JsonRpcResponse::error(
        id,
        -32000,
        "router policy did not produce an attempt",
    )))
}

fn router_failure_category(result: &Value) -> Option<FailureCategory> {
    router_diagnostic(result)
        .and_then(|diagnostic| diagnostic.get("failureCategory"))
        .and_then(Value::as_str)
        .and_then(|category| category.parse().ok())
}

fn router_wait_failure_category(wait_status: &Value) -> Option<FailureCategory> {
    wait_status
        .get("timedOut")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        .then_some(FailureCategory::RunnerTimeout)
}

fn router_stop_reason(result: &Value) -> Option<RouterStopReason> {
    let stop_reason = router_diagnostic(result)
        .and_then(|diagnostic| diagnostic.get("acpStopReason"))
        .and_then(Value::as_str)?;
    match stop_reason {
        "end_turn" => Some(RouterStopReason::EndTurn),
        "refusal" => Some(RouterStopReason::Refusal),
        "cancelled" => Some(RouterStopReason::Cancelled),
        _ => None,
    }
}

fn router_diagnostic(result: &Value) -> Option<&Value> {
    result.get("reviewPacket")?.get("diagnostic")
}

fn router_terminal_kind(disposition: AttemptDisposition) -> &'static str {
    match disposition {
        AttemptDisposition::TrustedFinal => "answer",
        AttemptDisposition::Blocker => "blocker",
        AttemptDisposition::FailoverEligible | AttemptDisposition::TerminalFailure => "failure",
    }
}

fn router_stop_reason_text(stop_reason: RouterStopReason) -> &'static str {
    match stop_reason {
        RouterStopReason::EndTurn => "end_turn",
        RouterStopReason::Refusal => "refusal",
        RouterStopReason::Cancelled => "cancelled",
    }
}

fn router_blocker_reason(
    disposition: AttemptDisposition,
    stop_reason: Option<RouterStopReason>,
    failure_category: Option<FailureCategory>,
) -> Option<&'static str> {
    if disposition != AttemptDisposition::Blocker {
        return None;
    }
    match stop_reason {
        Some(reason @ (RouterStopReason::Refusal | RouterStopReason::Cancelled)) => {
            Some(router_stop_reason_text(reason))
        }
        _ => failure_category.map(FailureCategory::as_str),
    }
}

async fn write_router_evidence_updates(
    stdout: &mut io::Stdout,
    session_id: &str,
    provider: ProviderKind,
    transcript: &Value,
) -> io::Result<()> {
    let Some(events) = transcript["events"].as_array() else {
        return Ok(());
    };
    let provider_events = events
        .iter()
        .filter(|event| event.get("kind").and_then(Value::as_str) == Some("provider_event"))
        .collect::<Vec<_>>();
    let truncated = provider_events.len() > ROUTER_EVIDENCE_UPDATE_LIMIT;
    for event in provider_events.iter().take(ROUTER_EVIDENCE_UPDATE_LIMIT) {
        let notification = JsonRpcNotification::new(
            "session/update",
            json!({
                "sessionId": session_id,
                "update": {
                    "sessionUpdate": "agent_bridge_evidence",
                    "agentBridgeEvidence": {
                        "type": "provider_internal",
                        "provider": provider,
                        "kind": event.get("kind").and_then(Value::as_str).unwrap_or("provider_event"),
                        "source": event.get("source").and_then(Value::as_str).unwrap_or("provider"),
                        "eventIndex": event.get("index").and_then(Value::as_u64).unwrap_or(0),
                        "bounded": {
                            "limit": ROUTER_EVIDENCE_UPDATE_LIMIT,
                            "truncated": truncated
                        }
                    }
                }
            }),
        );
        write_json_message(stdout, &notification).await?;
    }
    Ok(())
}

fn acp_prompt_text(prompt: &Value) -> Option<String> {
    if let Some(text) = prompt.as_str() {
        return Some(text.to_string());
    }
    let parts = prompt.as_array()?.iter().filter_map(|part| {
        if part.get("type").and_then(Value::as_str) == Some("text") {
            part.get("text").and_then(Value::as_str)
        } else {
            None
        }
    });
    let text = parts.collect::<Vec<_>>().join("\n");
    (!text.trim().is_empty()).then_some(text)
}

fn transcript_final_text(transcript: &Value) -> Option<String> {
    transcript["events"]
        .as_array()?
        .iter()
        .rev()
        .find_map(|event| {
            if event.get("kind").and_then(Value::as_str) == Some("provider_result") {
                event["parsed"]["result"].as_str().map(str::to_string)
            } else {
                None
            }
        })
}

fn exit_with_reload(cli_config: crate::config::ConfigCliOverrides) -> ! {
    match reload_server(cli_config) {
        Ok(pid) => {
            write_stdout_json(&json!({
                "status": "ok",
                "pid": pid,
                "signal": "SIGHUP",
            }));
            std::process::exit(0);
        }
        Err(error) => {
            write_stdout_json(&json!({
                "status": "error",
                "error": error,
            }));
            std::process::exit(1);
        }
    }
}

#[cfg(unix)]
fn reload_server(cli_config: crate::config::ConfigCliOverrides) -> Result<u32, String> {
    let state_dir = match crate::config::Config::from_env(cli_config.clone()) {
        Ok(config) => config.state_dir().to_path_buf(),
        Err(error) => {
            tracing::warn!(
                error = %error,
                "[agent-bridge] reload command could not parse full config; falling back to state-dir-only PID lookup"
            );
            crate::config::reload_pid_state_dir(&cli_config)
        }
    };
    let pid = read_pid(&state_dir.join("server.pid"))?;
    let result = unsafe { libc::kill(pid as libc::pid_t, libc::SIGHUP) };
    if result == 0 {
        Ok(pid)
    } else {
        Err(std::io::Error::last_os_error().to_string())
    }
}

#[cfg(not(unix))]
fn reload_server(_cli_config: crate::config::ConfigCliOverrides) -> Result<u32, String> {
    Err("reload is only supported on Unix platforms".to_string())
}

fn read_pid(path: &Path) -> Result<u32, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    text.trim()
        .parse::<u32>()
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

struct PidLock {
    path: PathBuf,
    pid: u32,
    registered: bool,
}

impl PidLock {
    fn acquire(state_dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(state_dir).map_err(|error| error.to_string())?;
        let path = state_dir.join("server.pid");
        if let Ok(pid) = read_pid(&path) {
            if process_holds_pid_lock(pid) {
                return Ok(Self {
                    path,
                    pid: std::process::id(),
                    registered: false,
                });
            }
            let _ = std::fs::remove_file(&path);
        }
        let pid = std::process::id();
        std::fs::write(&path, format!("{pid}\n")).map_err(|error| error.to_string())?;
        Ok(Self {
            path,
            pid,
            registered: true,
        })
    }
}

impl Drop for PidLock {
    fn drop(&mut self) {
        if self.registered && read_pid(&self.path).ok() == Some(self.pid) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
    result == 0 || std::io::Error::last_os_error().kind() == std::io::ErrorKind::PermissionDenied
}

#[cfg(not(unix))]
fn process_is_alive(_pid: u32) -> bool {
    false
}

#[cfg(unix)]
fn process_holds_pid_lock(pid: u32) -> bool {
    process_is_alive(pid) && process_command_is_agent_bridge(pid).unwrap_or(true)
}

#[cfg(not(unix))]
fn process_holds_pid_lock(pid: u32) -> bool {
    process_is_alive(pid)
}

#[cfg(unix)]
fn process_command_is_agent_bridge(pid: u32) -> Option<bool> {
    let output = std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "args="])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let command_line = String::from_utf8_lossy(&output.stdout);
    Some(command_line_is_agent_bridge(&command_line))
}

#[cfg(unix)]
fn command_line_is_agent_bridge(command_line: &str) -> bool {
    let Some(command) = command_line.split_whitespace().next() else {
        return false;
    };
    Path::new(command)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, "agent-bridge-mcp" | "agent-bridge-mcp-rs"))
}

#[cfg(unix)]
fn install_reload_handler(cli_config: crate::config::ConfigCliOverrides) {
    tokio::spawn(async move {
        let mut sighup = match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())
        {
            Ok(signal) => signal,
            Err(error) => {
                tracing::error!(error = %error, "[agent-bridge] failed to install SIGHUP handler");
                return;
            }
        };
        while sighup.recv().await.is_some() {
            match crate::config::reload_runtime_config(cli_config.clone()) {
                Ok(config) => tracing::info!(
                    workspace_count = config.configured_workspace_roots().len(),
                    "[agent-bridge] reloaded runtime config"
                ),
                Err(error) => tracing::error!(
                    error = %error,
                    "[agent-bridge] failed to reload runtime config; preserving incumbent workspace roots"
                ),
            }
        }
    });
}

#[cfg(not(unix))]
fn install_reload_handler(_cli_config: crate::config::ConfigCliOverrides) {}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .json()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_current_span(true)
        .with_span_list(true)
        .try_init();
}

fn exit_with_config_check(cli_config: crate::config::ConfigCliOverrides) -> ! {
    match crate::config::Config::from_env(cli_config) {
        Ok(config) => {
            write_stdout_json(&json!({
                "status": "ok",
                "valid": true,
                "workspaces": config.configured_workspace_roots()
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>(),
                "stateDir": config.state_dir().display().to_string(),
                "claudeHostSocket": config.claude_host_socket()
                    .map(|path| path.display().to_string()),
                "maxActiveTasks": config.max_active_tasks(),
            }));
            std::process::exit(0);
        }
        Err(error) => {
            write_stdout_json(&json!({
                "status": "error",
                "valid": false,
                "error": error,
            }));
            std::process::exit(1);
        }
    }
}

async fn exit_with_doctor_smoke(providers: Vec<crate::domain::ProviderKind>) -> ! {
    let provider_names = providers
        .iter()
        .map(|provider| provider.as_str())
        .collect::<Vec<_>>();
    let mut arguments = json!({
        "smoke": true,
    });
    if !provider_names.is_empty() {
        arguments["providers"] = json!(provider_names);
    }
    match crate::server::doctor_report(arguments).await {
        Ok(report) => {
            let ready = report["launchReadiness"]["status"].as_str() == Some("ready");
            write_stdout_json(&report);
            std::process::exit(if ready { 0 } else { 1 });
        }
        Err(error) => {
            write_stdout_json(&json!({
                "status": "error",
                "error": error,
            }));
            std::process::exit(1);
        }
    }
}

fn write_stdout_json(value: &Value) {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, value).expect("serialize CLI JSON");
    stdout.write_all(b"\n").expect("write CLI JSON newline");
    stdout.flush().expect("flush CLI JSON");
}

async fn run_until_shutdown() -> io::Result<i32> {
    tokio::select! {
        result = run_stdio_server() => result.map(|_| 0),
        exit_code = shutdown_signal() => {
            if let Ok(manager) = TaskManagerHandle::from_env().await {
                let _ = manager.shutdown().await;
            }
            Ok(exit_code)
        }
    }
}

async fn run_stdio_server() -> io::Result<()> {
    let stdin = io::stdin();
    let mut lines = BufReader::new(stdin).lines();
    let mut stdout = io::stdout();
    let mut completion_notifications = crate::task::subscribe_completion_notifications();

    loop {
        tokio::select! {
            line = lines.next_line() => {
                let Some(line) = line? else {
                    return Ok(());
                };
                if line.trim().is_empty() {
                    continue;
                }

                let request: Result<JsonRpcRequest, _> = serde_json::from_str(&line);
                match request {
                    Ok(request) => {
                        if let Some(response) = handle_request(request).await {
                            write_response(&mut stdout, &response).await?;
                        }
                    }
                    Err(_) => {
                        let response = JsonRpcResponse::error(JsonRpcId::Null, -32700, "Parse error");
                        write_response(&mut stdout, &response).await?;
                    }
                }
            }
            notification = completion_notifications.recv() => {
                if let Some(notification) = notification {
                    write_json_message(&mut stdout, &notification).await?;
                }
            }
        }
    }
}

#[cfg(unix)]
async fn shutdown_signal() -> i32 {
    use tokio::signal::unix::{SignalKind, signal};

    let mut sigterm = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => 130,
        _ = sigterm.recv() => 143,
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() -> i32 {
    let _ = tokio::signal::ctrl_c().await;
    130
}

async fn write_response(stdout: &mut io::Stdout, response: &JsonRpcResponse) -> io::Result<()> {
    write_json_message(stdout, response).await
}

async fn write_json_message<T: Serialize>(stdout: &mut io::Stdout, message: &T) -> io::Result<()> {
    let mut line = serde_json::to_vec(message).map_err(io::Error::other)?;
    line.push(b'\n');
    stdout.write_all(&line).await?;
    stdout.flush().await
}

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        tracing::error!(panic = %info, "[agent-bridge] panic {info}");
        // Best-effort: terminate any tracked provider children so a panic does
        // not leave orphaned processes behind.
        #[cfg(unix)]
        crate::task::terminate_all_active_pids(libc::SIGTERM);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn pid_lock_reclaims_pid_file_for_unrelated_live_process() {
        let state_dir = temp_dir("pid-lock-unrelated-live-process");
        std::fs::write(
            state_dir.join("server.pid"),
            format!("{}\n", std::process::id()),
        )
        .unwrap();

        let lock = PidLock::acquire(&state_dir).unwrap();

        assert_eq!(read_pid(&state_dir.join("server.pid")).unwrap(), lock.pid);
    }

    #[cfg(unix)]
    #[test]
    fn command_line_match_requires_agent_bridge_mcp_executable() {
        assert!(command_line_is_agent_bridge(
            "/Users/pedro/.local/bin/agent-bridge-mcp"
        ));
        assert!(command_line_is_agent_bridge("agent-bridge-mcp"));
        assert!(!command_line_is_agent_bridge(
            "/tmp/agent_bridge_mcp-test-binary"
        ));
        assert!(command_line_is_agent_bridge(
            "/Users/pedro/.local/bin/agent-bridge-mcp-rs"
        ));
    }

    fn temp_dir(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "agent-bridge-runtime-{label}-{}",
            Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }
}
