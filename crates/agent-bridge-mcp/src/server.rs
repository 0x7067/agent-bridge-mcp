use crate::domain::{Isolation, ProviderKind, TimeoutSeconds, WorktreeName};
use crate::guidance;
use crate::mcp::{JsonRpcId, JsonRpcRequest, JsonRpcResponse};
use crate::provider::{self, ProviderTask};
use crate::task::{TaskManagerHandle, validate_registry_text};
use crate::tools::{TaskPreviewInput, ToolCallParams, ToolName, tool_definitions};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinSet;
use tokio::time::{Duration, Instant, timeout};
mod diagnostics;
use diagnostics::{doctor, observe_task_extension_metadata};

const PROTOCOL_VERSION: &str = "2024-11-05";
const MAX_PROMPT_BYTES: usize = 100 * 1024;
const VERSION_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_AGGREGATE_TIMEOUT_MS: u64 = 110_000;
const MAX_AGGREGATE_TIMEOUT_MS: i64 = 120_000;
const MAX_PROVIDER_TIMEOUT_MS: i64 = 90_000;
const CHILD_SHUTDOWN_GRACE: Duration = Duration::from_millis(500);
const MAX_CLIENT_CONFIG_BYTES: u64 = 1024 * 1024;
const MAX_BINARY_FINGERPRINT_BYTES: u64 = 16 * 1024 * 1024;
const TASKS_EXTENSION_ID: &str = "io.modelcontextprotocol/tasks";

pub async fn handle_request(request: JsonRpcRequest) -> Option<JsonRpcResponse> {
    request.id.as_ref()?;
    observe_task_extension_metadata(&request);
    let id = request.id.clone().unwrap_or(JsonRpcId::Null);
    let response = match request.method.as_str() {
        "initialize" => JsonRpcResponse::result(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {}, "prompts": {}, "resources": {} },
                "serverInfo": { "name": "agent-bridge-mcp", "version": "0.1.0" },
                "instructions": guidance::INITIALIZATION_INSTRUCTIONS
            }),
        ),
        "tools/list" => JsonRpcResponse::result(id, json!({ "tools": tool_definitions() })),
        "prompts/list" => {
            JsonRpcResponse::result(id, json!({ "prompts": guidance::prompt_definitions() }))
        }
        "prompts/get" => match guidance::get_prompt(request.params.unwrap_or_else(|| json!({}))) {
            Ok(result) => JsonRpcResponse::result(id, result),
            Err(error) => JsonRpcResponse::error(id, -32602, error),
        },
        "resources/list" => {
            JsonRpcResponse::result(id, json!({ "resources": guidance::resource_definitions() }))
        }
        "resources/read" => {
            match guidance::read_resource(request.params.unwrap_or_else(|| json!({}))) {
                Ok(result) => JsonRpcResponse::result(id, result),
                Err(error) => JsonRpcResponse::error(id, -32002, error),
            }
        }
        "tools/call" => JsonRpcResponse::result(
            id,
            call_tool(request.params.unwrap_or_else(|| json!({}))).await,
        ),
        _ => JsonRpcResponse::error(id, -32601, format!("Method not found: {}", request.method)),
    };
    Some(response)
}

async fn call_tool(params: Value) -> Value {
    let parsed: Result<ToolCallParams, _> = serde_json::from_value(params);
    let params = match parsed {
        Ok(params) => params,
        Err(error) => return tool_error(error.to_string()),
    };
    if let Err(error) = reject_unknown_arguments(params.name, &params.arguments) {
        return tool_error(error);
    }
    match params.name {
        ToolName::ProvidersList => tool_json(json!({ "providers": provider::capabilities() })),
        ToolName::Doctor => tool_result(doctor(params.arguments).await),
        ToolName::AgentSpawn => {
            let dry_run = params
                .arguments
                .get("dryRun")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if dry_run {
                // dryRun subsumes the former agent_preview tool.
                match task_preview(params.arguments) {
                    Ok(payload) => tool_json(payload),
                    Err(error) => tool_error(error),
                }
            } else {
                let mut args = params.arguments.clone();
                if let Some(object) = args.as_object_mut() {
                    object.remove("dryRun");
                }
                match TaskManagerHandle::from_env().await {
                    Ok(manager) => tool_result(manager.spawn(args).await),
                    Err(error) => tool_error(error),
                }
            }
        }
        ToolName::AgentList => match TaskManagerHandle::from_env().await {
            Ok(manager) => tool_result(agent_list(manager, params.arguments).await),
            Err(error) => tool_error(error),
        },
        ToolName::AgentObserve => handle_agent_observe(params.arguments).await,
        ToolName::AgentResult => handle_agent_result(params.arguments).await,
        ToolName::AgentStop => match (
            require_agent_id(&params.arguments),
            TaskManagerHandle::from_env().await,
        ) {
            (Ok(agent_id), Ok(manager)) => tool_result(manager.stop(agent_id).await),
            (Err(error), _) | (_, Err(error)) => tool_error(error),
        },
        ToolName::AgentRemove => match (
            require_agent_id(&params.arguments),
            TaskManagerHandle::from_env().await,
        ) {
            (Ok(agent_id), Ok(manager)) => tool_result(manager.remove(agent_id).await),
            (Err(error), _) | (_, Err(error)) => tool_error(error),
        },
    }
}

/// Handles `agent_observe`, which subsumes the former agent_status (limit:0),
/// agent_wait (until:"final"), and agent_transcript (events) tools.
async fn handle_agent_observe(arguments: Value) -> Value {
    let detailed = is_detailed(&arguments);
    let until = arguments
        .get("until")
        .and_then(Value::as_str)
        .unwrap_or("now");
    let cursor = arguments.get("cursor").and_then(Value::as_u64);
    let limit = arguments.get("limit").and_then(Value::as_u64);
    let timeout_ms = arguments.get("timeoutMs").and_then(Value::as_i64);
    match (
        require_agent_id(&arguments),
        TaskManagerHandle::from_env().await,
    ) {
        (Ok(agent_id), Ok(manager)) => {
            if until == "final" {
                tool_result(manager.wait(agent_id, timeout_ms, detailed).await)
            } else if limit == Some(0) {
                tool_result(manager.status(agent_id, detailed).await)
            } else {
                tool_result(
                    manager
                        .observe(agent_id, cursor, limit, timeout_ms, detailed)
                        .await,
                )
            }
        }
        (Err(error), _) | (_, Err(error)) => tool_error(error),
    }
}

/// Handles `agent_result`, which subsumes the former agent_logs tool via sections.
async fn handle_agent_result(arguments: Value) -> Value {
    let detailed = is_detailed(&arguments);
    let sections = result_sections(&arguments);
    let max_bytes = arguments.get("maxBytes").and_then(Value::as_i64);
    let stdout_line = arguments
        .get("stdoutLine")
        .and_then(Value::as_u64)
        .map(|value| value as usize);
    let stderr_line = arguments
        .get("stderrLine")
        .and_then(Value::as_u64)
        .map(|value| value as usize);
    let cursor = arguments.get("cursor").and_then(Value::as_u64);
    let limit = arguments.get("limit").and_then(Value::as_u64);
    match (
        require_agent_id(&arguments),
        TaskManagerHandle::from_env().await,
    ) {
        (Ok(agent_id), Ok(manager)) => tool_result(
            manager
                .result(
                    agent_id, sections, max_bytes, stdout_line, stderr_line, cursor, limit,
                    detailed,
                )
                .await,
        ),
        (Err(error), _) | (_, Err(error)) => tool_error(error),
    }
}

fn is_detailed(arguments: &Value) -> bool {
    arguments.get("verbosity").and_then(Value::as_str) == Some("detailed")
}

fn result_sections(arguments: &Value) -> crate::task::ResultSections {
    match arguments.get("sections").and_then(Value::as_array) {
        Some(items) => {
            crate::task::ResultSections::from_names(items.iter().filter_map(Value::as_str))
        }
        None => crate::task::ResultSections::default_sections(),
    }
}

fn reject_unknown_arguments(name: ToolName, arguments: &Value) -> Result<(), String> {
    let allowed = match name {
        ToolName::ProvidersList => &[][..],
        ToolName::Doctor => &[
            "focus",
            "smoke",
            "timeoutMs",
            "providers",
            "aggregateTimeoutMs",
            "providerTimeoutMs",
            "cwd",
        ][..],
        ToolName::AgentSpawn => &[
            "provider",
            "mode",
            "prompt",
            "title",
            "cwd",
            "timeoutSeconds",
            "model",
            "effort",
            "thinking",
            "isolation",
            "worktreeName",
            "profile",
            "dryRun",
        ][..],
        ToolName::AgentList => &[
            "status",
            "provider",
            "mode",
            "cwd",
            "titleContains",
            "limit",
        ][..],
        ToolName::AgentStop | ToolName::AgentRemove => &["agentId"][..],
        ToolName::AgentObserve => &[
            "agentId",
            "until",
            "cursor",
            "limit",
            "timeoutMs",
            "verbosity",
        ][..],
        ToolName::AgentResult => &[
            "agentId",
            "sections",
            "maxBytes",
            "stdoutLine",
            "stderrLine",
            "cursor",
            "limit",
            "verbosity",
        ][..],
    };
    let Some(object) = arguments.as_object() else {
        return Ok(());
    };
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!(
                "Unknown argument for {}: {key}",
                tool_name_str(name)
            ));
        }
    }
    Ok(())
}

fn tool_name_str(name: ToolName) -> &'static str {
    match name {
        ToolName::ProvidersList => "providers_list",
        ToolName::Doctor => "doctor",
        ToolName::AgentSpawn => "agent_spawn",
        ToolName::AgentList => "agent_list",
        ToolName::AgentObserve => "agent_observe",
        ToolName::AgentResult => "agent_result",
        ToolName::AgentStop => "agent_stop",
        ToolName::AgentRemove => "agent_remove",
    }
}

async fn agent_list(manager: TaskManagerHandle, arguments: Value) -> Result<Value, String> {
    let raw = manager.list(agent_list_arguments(arguments)?).await?;
    Ok(agent_list_response(raw))
}

fn agent_list_arguments(arguments: Value) -> Result<Value, String> {
    let mut object = match arguments {
        Value::Null => serde_json::Map::new(),
        Value::Object(object) => object,
        _ => return Err("agent_list arguments must be an object".to_string()),
    };
    object.insert("presentation".to_string(), json!(true));
    object.insert("scope".to_string(), json!("active_recent"));
    Ok(Value::Object(object))
}

fn agent_list_response(mut raw: Value) -> Value {
    let Some(object) = raw.as_object_mut() else {
        return raw;
    };
    let agents = object.remove("tasks").unwrap_or_else(|| json!([]));
    object.remove("presentation");
    object.insert("agents".to_string(), agents);
    raw
}

struct ProbeResult {
    status: Option<std::process::ExitStatus>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    failure_category: Option<&'static str>,
    error: Option<String>,
    duration_ms: u64,
}

async fn run_probe(
    command: &provider::ProviderCommand,
    provider: ProviderKind,
    timeout_ms: u64,
    phase: &'static str,
) -> ProbeResult {
    if provider == ProviderKind::Claude
        && let (Some(socket_path), Some(claude_command)) = (
            crate::claude_host::socket_path_from_env(),
            command.claude_host.as_ref(),
        )
    {
        let started = Instant::now();
        return match crate::claude_host::run_claude(&socket_path, claude_command).await {
            Ok(response) => host_probe_result(response, started.elapsed().as_millis() as u64),
            Err(error) => ProbeResult {
                status: None,
                stdout: Vec::new(),
                stderr: Vec::new(),
                failure_category: Some("host_runner_unavailable"),
                error: Some(error),
                duration_ms: started.elapsed().as_millis() as u64,
            },
        };
    }
    let started = Instant::now();
    let mut process = tokio::process::Command::new(&command.command);
    process
        .args(&command.args)
        .current_dir(&command.cwd)
        .env_clear()
        .envs(provider::provider_env(provider))
        .stdin(if command.stdin.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    configure_child_process_group(&mut process);
    let child = process.spawn().map_err(|error| ProbeResult {
        status: None,
        stdout: Vec::new(),
        stderr: Vec::new(),
        failure_category: Some("provider_start_error"),
        error: Some(error.to_string()),
        duration_ms: started.elapsed().as_millis() as u64,
    });
    let mut child = match child {
        Ok(child) => child,
        Err(result) => return result,
    };
    let pid = child.id();
    let stdin_write = if let (Some(stdin), Some(mut child_stdin)) =
        (command.stdin.as_deref(), child.stdin.take())
    {
        child_stdin.write_all(stdin.as_bytes()).await
    } else {
        Ok(())
    };
    if let Err(error) = stdin_write {
        return ProbeResult {
            status: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
            failure_category: Some("provider_start_error"),
            error: Some(error.to_string()),
            duration_ms: started.elapsed().as_millis() as u64,
        };
    }
    let stdout_task = child.stdout.take().map(|mut stdout| {
        tokio::spawn(async move {
            let mut bytes = Vec::new();
            let _ = stdout.read_to_end(&mut bytes).await;
            bytes
        })
    });
    let stderr_task = child.stderr.take().map(|mut stderr| {
        tokio::spawn(async move {
            let mut bytes = Vec::new();
            let _ = stderr.read_to_end(&mut bytes).await;
            bytes
        })
    });
    let wait = timeout(Duration::from_millis(timeout_ms), child.wait()).await;
    let (status, failure_category, error) = match wait {
        Ok(Ok(status)) => {
            let failure_category = if status.success() {
                None
            } else {
                Some("provider_exit_error")
            };
            (Some(status), failure_category, None)
        }
        Ok(Err(error)) => (None, Some("provider_exit_error"), Some(error.to_string())),
        Err(_) => {
            terminate_child_tree(pid, libc::SIGTERM);
            let status = match timeout(CHILD_SHUTDOWN_GRACE, child.wait()).await {
                Ok(result) => result.ok(),
                Err(_) => {
                    terminate_child_tree(pid, libc::SIGKILL);
                    child.wait().await.ok()
                }
            };
            eprintln!(
                "[agent-bridge] provider probe timeout provider={} phase={} elapsedMs={} timeoutMs={} failureCategory=provider_timeout",
                provider.as_str(),
                phase,
                started.elapsed().as_millis(),
                timeout_ms
            );
            (
                status,
                Some("provider_timeout"),
                Some(format!("command timed out after {timeout_ms}ms")),
            )
        }
    };
    let stdout = match stdout_task {
        Some(task) => task.await.unwrap_or_default(),
        None => Vec::new(),
    };
    let stderr = match stderr_task {
        Some(task) => task.await.unwrap_or_default(),
        None => Vec::new(),
    };
    ProbeResult {
        status,
        stdout,
        stderr,
        failure_category,
        error,
        duration_ms: started.elapsed().as_millis() as u64,
    }
}

fn host_probe_result(response: crate::claude_host::HostResponse, duration_ms: u64) -> ProbeResult {
    if !response.ok {
        return ProbeResult {
            status: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
            failure_category: Some("host_runner_unavailable"),
            error: response.error.map(|error| error.message),
            duration_ms,
        };
    }
    match response.result {
        Some(crate::claude_host::HostResult::Run(run)) => {
            let crate::claude_host::HostRunResult {
                exit_code,
                signal,
                failure_category,
                result,
                pty_output_excerpt,
                duration_ms,
                ..
            } = *run;
            let success_text = result
                .as_ref()
                .map(|result| result.final_text.clone())
                .unwrap_or_default();
            ProbeResult {
                status: host_exit_status(exit_code, signal.as_deref()),
                stdout: success_text.into_bytes(),
                stderr: pty_output_excerpt.into_bytes(),
                failure_category: failure_category.as_deref().map(|category| match category {
                    "provider_timeout" => "provider_timeout",
                    "provider_exit_error" => "provider_exit_error",
                    "client_disconnected" => "provider_timeout",
                    _ => "provider_output_error",
                }),
                error: failure_category,
                duration_ms,
            }
        }
        _ => ProbeResult {
            status: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
            failure_category: Some("host_runner_unavailable"),
            error: Some("host runner returned unexpected response".to_string()),
            duration_ms,
        },
    }
}

#[cfg(unix)]
fn host_exit_status(
    exit_code: Option<i32>,
    signal: Option<&str>,
) -> Option<std::process::ExitStatus> {
    use std::os::unix::process::ExitStatusExt;
    if let Some(code) = exit_code {
        return Some(std::process::ExitStatus::from_raw(code << 8));
    }
    let signal = signal.and_then(|signal| signal.strip_prefix("SIG"));
    let number = match signal {
        Some("TERM") => libc::SIGTERM,
        Some("KILL") => libc::SIGKILL,
        _ => return None,
    };
    Some(std::process::ExitStatus::from_raw(number))
}

#[cfg(not(unix))]
fn host_exit_status(
    _exit_code: Option<i32>,
    _signal: Option<&str>,
) -> Option<std::process::ExitStatus> {
    None
}

#[cfg(unix)]
fn configure_child_process_group(command: &mut tokio::process::Command) {
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
fn configure_child_process_group(_command: &mut tokio::process::Command) {}

#[cfg(unix)]
fn terminate_child_tree(pid: Option<u32>, signal: i32) {
    if let Some(pid) = pid {
        unsafe {
            libc::killpg(pid as libc::pid_t, signal);
        }
    }
}

#[cfg(not(unix))]
fn terminate_child_tree(_pid: Option<u32>, _signal: i32) {}

fn probe_error_text(output: &ProbeResult) -> String {
    if let Some(error) = output.error.as_deref() {
        return error.to_string();
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return stderr;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        return stdout;
    }
    "provider command failed".to_string()
}

fn provider_diagnostic(
    provider: ProviderKind,
    command: &provider::ProviderCommand,
    output: &ProbeResult,
    timeout_ms: u64,
    startup_verified: bool,
    phase: &'static str,
) -> Value {
    let redactions = diagnostic_redactions(command);
    let mut diagnostic = json!({
        "failureCategory": output.failure_category.unwrap_or("provider_output_error"),
        "provider": provider.as_str(),
        "commandKind": command_kind(provider, command),
        "commandPath": command_path(provider, command),
        "launchStrategy": launch_strategy(provider),
        "startupVerified": startup_verified,
        "timeoutMs": timeout_ms,
        "elapsedMs": output.duration_ms,
        "phase": phase,
        "exitCode": output.status.as_ref().and_then(|status| status.code()),
        "signal": signal_name(output.status.as_ref()),
        "stdoutExcerpt": excerpt(&output.stdout, &redactions),
        "stderrExcerpt": excerpt(&output.stderr, &redactions)
    });
    if provider == ProviderKind::Claude && crate::claude_host::socket_path_from_env().is_none() {
        diagnostic["recommendation"] = json!(
            "Start the Agent Bridge Claude host runner and retry doctor with focus: providers and smoke: true"
        );
    }
    diagnostic
}

fn diagnostic_redactions(command: &provider::ProviderCommand) -> Vec<String> {
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

fn smoke_output_is_accepted(provider: ProviderKind, stdout: &[u8]) -> bool {
    let text = String::from_utf8_lossy(stdout);
    match provider {
        ProviderKind::Claude | ProviderKind::Codex => text.lines().any(|line| {
            serde_json::from_str::<Value>(line.trim())
                .ok()
                .and_then(|value| {
                    value
                        .get("result")
                        .or_else(|| value.get("output"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .is_some_and(|result| result.contains(provider::PROVIDER_SMOKE_TOKEN))
                || line.contains(provider::PROVIDER_SMOKE_TOKEN)
        }),
        ProviderKind::Cursor | ProviderKind::Kimi | ProviderKind::Antigravity => {
            text.contains(provider::PROVIDER_SMOKE_TOKEN)
        }
    }
}

fn command_kind(provider: ProviderKind, command: &provider::ProviderCommand) -> String {
    if provider != ProviderKind::Claude {
        return provider.as_str().to_string();
    }
    command
        .command_kind
        .as_deref()
        .unwrap_or("owned-interactive-claude")
        .to_string()
}

fn command_path(provider: ProviderKind, command: &provider::ProviderCommand) -> String {
    if provider == ProviderKind::Claude && command.command == "/bin/zsh" {
        return command
            .args
            .get(3)
            .cloned()
            .unwrap_or_else(|| command.command.clone());
    }
    command.command.clone()
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

fn excerpt(bytes: &[u8], redactions: &[String]) -> String {
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

#[cfg(unix)]
fn signal_name(status: Option<&std::process::ExitStatus>) -> Option<String> {
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
fn signal_name(_status: Option<&std::process::ExitStatus>) -> Option<String> {
    None
}

fn default_cwd() -> String {
    env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .display()
        .to_string()
}

fn task_preview(arguments: Value) -> Result<Value, String> {
    let input: TaskPreviewInput =
        serde_json::from_value(arguments).map_err(|error| error.to_string())?;
    validate_preview_input(&input)?;
    let timeout = TimeoutSeconds::new(input.timeout_seconds);
    let cwd = safe_cwd(input.cwd.as_deref())?;
    let task = ProviderTask {
        provider: input.provider,
        mode: input.mode,
        prompt: &input.prompt,
        title: input.title.as_deref(),
        cwd: &cwd,
        timeout_seconds: timeout.get(),
        model: input.model.as_deref(),
        effort: input.effort.as_deref(),
        thinking: input.thinking.as_deref(),
        profile: input
            .profile
            .unwrap_or(crate::domain::LaunchProfile::Bridge),
    };
    let command = provider::build_command(&task)?;
    let env = provider::provider_env(input.provider);
    let args: Vec<String> = command
        .args
        .into_iter()
        .map(|arg| {
            if arg.contains(&input.prompt) {
                "<prompt redacted>".to_string()
            } else {
                arg
            }
        })
        .collect();
    let env_keys: Vec<String> = env.keys().cloned().collect();
    let mut preview = json!({
        "command": command.command,
        "cwd": command.cwd,
        "timeoutSeconds": command.timeout_seconds,
        "args": args,
        "stdin": command.stdin.as_ref().map(|_| "<prompt redacted>"),
        "envKeys": env_keys,
        "profile": command.profile,
        "promptStrategy": command.prompt_strategy,
        "profileDiagnostics": command.profile_diagnostics
    });
    if input.provider == ProviderKind::Claude {
        preview["launchStrategy"] = json!(launch_strategy(input.provider));
    }
    Ok(preview)
}

fn validate_preview_input(input: &TaskPreviewInput) -> Result<(), String> {
    if input.prompt.is_empty() {
        return Err("prompt is required".to_string());
    }
    if input.prompt.len() > MAX_PROMPT_BYTES {
        return Err(format!("prompt exceeds {MAX_PROMPT_BYTES} bytes"));
    }
    if let Some(name) = input.worktree_name.as_deref() {
        WorktreeName::new(name)?;
    }
    let isolation = input.isolation.unwrap_or(Isolation::None);
    if !matches!(isolation, Isolation::None | Isolation::Worktree) {
        return Err("isolation must be one of: none, worktree".to_string());
    }
    Ok(())
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
        .any(|root| is_inside(&real_cwd, root))
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

fn is_inside(candidate: &Path, root: &Path) -> bool {
    candidate == root || candidate.strip_prefix(root).is_ok()
}

fn tool_json(value: Value) -> Value {
    let text = serde_json::to_string_pretty(&value).unwrap();
    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": value,
        "isError": false
    })
}

fn tool_result(result: Result<Value, String>) -> Value {
    match result {
        Ok(value) => tool_json(value),
        Err(error) => tool_error(error),
    }
}

fn tool_error(error: impl Into<String>) -> Value {
    json!({
        "content": [{ "type": "text", "text": error.into() }],
        "isError": true
    })
}

fn require_agent_id(arguments: &Value) -> Result<String, String> {
    arguments
        .get("agentId")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "agentId is required".to_string())
}

#[cfg(test)]
mod tests {
    use super::diagnostics::*;
    use super::*;

    #[test]
    fn doctor_host_runner_response_reports_successful_ping_metadata() {
        let report = doctor_claude_host_runner_response(
            Path::new("/tmp/agent-bridge-host.sock"),
            12,
            crate::claude_host::HostResponse {
                version: 1,
                ok: true,
                result: Some(crate::claude_host::HostResult::Pong {
                    protocol_version: 1,
                    workspace_policy_id: "fixture-policy".to_string(),
                    ready: true,
                }),
                error: None,
            },
        );

        assert_eq!(report["status"], "ok");
        assert_eq!(report["protocolVersion"], 1);
        assert_eq!(report["workspacePolicyId"], "fixture-policy");
        assert_eq!(report["ready"], true);
    }

    #[test]
    fn doctor_host_runner_response_reports_protocol_and_workspace_mismatch() {
        let protocol = doctor_claude_host_runner_response(
            Path::new("/tmp/agent-bridge-host.sock"),
            12,
            crate::claude_host::HostResponse {
                version: 1,
                ok: false,
                result: None,
                error: Some(crate::claude_host::HostError {
                    code: "protocol_mismatch".to_string(),
                    message: "unsupported protocol version".to_string(),
                }),
            },
        );
        assert_eq!(protocol["status"], "error");
        assert_eq!(protocol["errorCode"], "protocol_mismatch");

        let workspace = doctor_claude_host_runner_response(
            Path::new("/tmp/agent-bridge-host.sock"),
            12,
            crate::claude_host::HostResponse {
                version: 1,
                ok: false,
                result: None,
                error: Some(crate::claude_host::HostError {
                    code: "workspace_policy_mismatch".to_string(),
                    message: "workspace policy mismatch".to_string(),
                }),
            },
        );
        assert_eq!(workspace["status"], "error");
        assert_eq!(workspace["errorCode"], "workspace_policy_mismatch");
        let recommendations = doctor_recommendations(
            "ok",
            "ok",
            "ok",
            "error",
            &json!({}),
            &json!({}),
            &json!({}),
        );
        assert!(
            recommendations
                .as_array()
                .unwrap()
                .iter()
                .any(|recommendation| recommendation["message"]
                    .as_str()
                    .unwrap()
                    .contains("AGENT_BRIDGE_WORKSPACES"))
        );
    }

    #[test]
    fn parses_codex_agent_bridge_section_and_env_keys() {
        let registration = parse_codex_registration(
            r#"
[mcp_servers."agent-bridge"]
command = "/tmp/agent-bridge-mcp" # comment
args = ["--stdio", "extra"]
env = { AGENT_BRIDGE_WORKSPACES = "/tmp/work", ANTHROPIC_API_KEY = "secret" }

[mcp_servers."agent-bridge".env]
CLAUDE_CODE_OAUTH_TOKEN = "secret"
"#,
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            registration.command.as_deref(),
            Some("/tmp/agent-bridge-mcp")
        );
        assert_eq!(registration.args, vec!["--stdio", "extra"]);
        assert!(
            registration
                .env_keys
                .contains(&"AGENT_BRIDGE_WORKSPACES".to_string())
        );
        assert!(
            registration
                .env_keys
                .contains(&"ANTHROPIC_API_KEY".to_string())
        );
        assert!(
            registration
                .env_keys
                .contains(&"CLAUDE_CODE_OAUTH_TOKEN".to_string())
        );
    }

    #[test]
    fn parses_json_agent_bridge_registration() {
        let registration = parse_json_registration(
            r#"{
  "mcpServers": {
    "agent-bridge": {
      "command": "/tmp/agent-bridge-mcp",
      "args": ["--stdio"],
      "env": {"TOKEN": "secret"}
    }
  }
}"#,
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            registration.command.as_deref(),
            Some("/tmp/agent-bridge-mcp")
        );
        assert_eq!(registration.args, vec!["--stdio"]);
        assert_eq!(registration.env_keys, vec!["TOKEN"]);
    }

    #[test]
    fn command_diagnostics_classify_missing_absolute_and_path_lookup() {
        let missing = command_diagnostic(None);
        assert_eq!(missing["resolution"], "missing");
        assert_eq!(missing["status"], "warning");

        let absolute_missing = command_diagnostic(Some("/tmp/agent-bridge-missing-binary"));
        assert_eq!(absolute_missing["resolution"], "absolute_missing");
        assert_eq!(absolute_missing["status"], "warning");

        let path_lookup = command_diagnostic(Some("agent-bridge-mcp"));
        assert_eq!(path_lookup["resolution"], "path_lookup_required");
        assert_eq!(path_lookup["status"], "info");
    }

    #[test]
    fn client_diagnostics_from_home_do_not_expose_env_values() {
        let root = std::env::temp_dir().join(format!(
            "agent-bridge-client-diagnostics-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(root.join(".cursor")).unwrap();
        std::fs::write(
            root.join(".cursor/mcp.json"),
            r#"{"mcpServers":{"agent-bridge":{"command":"agent-bridge-mcp","env":{"API_KEY":"secret-value"}}}}"#,
        )
        .unwrap();

        let clients = doctor_clients_from_home(&root);
        assert_eq!(clients["cursor"]["envKeys"], json!(["API_KEY"]));
        let serialized = serde_json::to_string(&clients).unwrap();
        assert!(!serialized.contains("secret-value"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn binary_report_classifies_matching_and_differing_files() {
        let root = std::env::temp_dir().join(format!(
            "agent-bridge-binary-report-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let running = root.join("running");
        let installed = root.join("installed");
        let release = root.join("release");
        std::fs::write(&running, "same").unwrap();
        std::fs::write(&installed, "same").unwrap();
        std::fs::write(&release, "same").unwrap();

        let matching = binary_report(
            BinaryTarget::inspect(running.clone()),
            BinaryTarget::inspect(installed.clone()),
            BinaryTarget::inspect(release.clone()),
        );
        assert_eq!(matching["status"], "ok");
        assert_eq!(matching["installed"]["matchesRelease"], true);
        assert_eq!(matching["installed"]["matchesRunning"], true);

        std::fs::write(&release, "different").unwrap();
        let differing = binary_report(
            BinaryTarget::inspect(running),
            BinaryTarget::inspect(installed),
            BinaryTarget::inspect(release),
        );
        assert_eq!(differing["status"], "warning");
        assert_eq!(differing["installed"]["matchesRelease"], false);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn binary_report_classifies_missing_and_oversized_files() {
        let root = std::env::temp_dir().join(format!(
            "agent-bridge-binary-missing-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let running = root.join("running");
        let installed = root.join("installed");
        let release = root.join("release");
        std::fs::write(&running, "same").unwrap();
        std::fs::write(&installed, "same").unwrap();

        let missing = binary_report(
            BinaryTarget::inspect(running.clone()),
            BinaryTarget::inspect(installed.clone()),
            BinaryTarget::inspect(release.clone()),
        );
        assert_eq!(missing["status"], "warning");
        assert_eq!(missing["release"]["exists"], false);

        let large = std::fs::File::create(&release).unwrap();
        large.set_len(MAX_BINARY_FINGERPRINT_BYTES + 1).unwrap();
        let oversized = binary_report(
            BinaryTarget::inspect(running),
            BinaryTarget::inspect(installed),
            BinaryTarget::inspect(release),
        );
        assert_eq!(oversized["status"], "warning");
        assert_eq!(
            oversized["release"]["fingerprintStatus"],
            "skipped_too_large"
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn binary_report_classifies_not_regular_file_as_error() {
        let root = std::env::temp_dir().join(format!(
            "agent-bridge-binary-error-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let running = root.join("running");
        let installed = root.join("installed");
        let release_dir = root.join("release-dir");
        std::fs::write(&running, "same").unwrap();
        std::fs::write(&installed, "same").unwrap();
        std::fs::create_dir_all(&release_dir).unwrap();

        let report = binary_report(
            BinaryTarget::inspect(running),
            BinaryTarget::inspect(installed),
            BinaryTarget::inspect(release_dir),
        );
        assert_eq!(report["release"]["fingerprintStatus"], "error");
        assert_eq!(report["release"]["readable"], false);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn release_binary_path_uses_doctor_cwd_when_no_override() {
        let root = std::env::temp_dir().join(format!(
            "agent-bridge-release-path-{}",
            uuid::Uuid::new_v4()
        ));
        let expected = root.join("target/release/agent-bridge-mcp");
        assert_eq!(release_binary_path(Some(root.to_str().unwrap())), expected);
    }
}
