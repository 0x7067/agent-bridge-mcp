use crate::domain::{Isolation, ProviderKind, TimeoutSeconds, WorktreeName};
use crate::mcp::{JsonRpcId, JsonRpcRequest, JsonRpcResponse};
use crate::provider::{self, ProviderTask};
use crate::task::TaskManagerHandle;
use crate::tools::{TaskPreviewInput, ToolCallParams, ToolName, tool_definitions};
use serde::Deserialize;
use serde_json::{Value, json};
use std::env;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{Duration, timeout};

const PROTOCOL_VERSION: &str = "2024-11-05";
const MAX_PROMPT_BYTES: usize = 100 * 1024;

pub async fn handle_request(request: JsonRpcRequest) -> Option<JsonRpcResponse> {
    request.id.as_ref()?;
    let id = request.id.clone().unwrap_or(JsonRpcId::Null);
    let response = match request.method.as_str() {
        "initialize" => JsonRpcResponse::result(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "agent-bridge-mcp", "version": "0.1.0" }
            }),
        ),
        "tools/list" => JsonRpcResponse::result(id, json!({ "tools": tool_definitions() })),
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
    match parsed {
        Ok(params) => match params.name {
            name if reject_unknown_arguments(name, &params.arguments).is_err() => {
                tool_error(reject_unknown_arguments(name, &params.arguments).unwrap_err())
            }
            ToolName::ProvidersList => tool_json(json!({ "providers": provider::capabilities() })),
            ToolName::ProvidersCheck => tool_json(providers_check(params.arguments).await),
            ToolName::TaskPreview => match task_preview(params.arguments) {
                Ok(payload) => tool_json(payload),
                Err(error) => tool_error(error),
            },
            ToolName::TaskSpawn => match TaskManagerHandle::from_env().await {
                Ok(manager) => tool_result(manager.spawn(params.arguments).await),
                Err(error) => tool_error(error),
            },
            ToolName::TaskList => match TaskManagerHandle::from_env().await {
                Ok(manager) => tool_result(manager.list().await),
                Err(error) => tool_error(error),
            },
            ToolName::TaskStatus => {
                match (
                    require_task_id(&params.arguments),
                    TaskManagerHandle::from_env().await,
                ) {
                    (Ok(task_id), Ok(manager)) => tool_result(manager.status(task_id).await),
                    (Err(error), _) | (_, Err(error)) => tool_error(error),
                }
            }
            ToolName::TaskWait => {
                let timeout_ms = params.arguments.get("timeoutMs").and_then(Value::as_i64);
                match (
                    require_task_id(&params.arguments),
                    TaskManagerHandle::from_env().await,
                ) {
                    (Ok(task_id), Ok(manager)) => {
                        tool_result(manager.wait(task_id, timeout_ms).await)
                    }
                    (Err(error), _) | (_, Err(error)) => tool_error(error),
                }
            }
            ToolName::TaskLogs => {
                let max_bytes = params.arguments.get("maxBytes").and_then(Value::as_i64);
                let stdout_line = params
                    .arguments
                    .get("stdoutLine")
                    .and_then(Value::as_u64)
                    .map(|value| value as usize);
                let stderr_line = params
                    .arguments
                    .get("stderrLine")
                    .and_then(Value::as_u64)
                    .map(|value| value as usize);
                match (
                    require_task_id(&params.arguments),
                    TaskManagerHandle::from_env().await,
                ) {
                    (Ok(task_id), Ok(manager)) => tool_result(
                        manager
                            .logs(task_id, max_bytes, stdout_line, stderr_line)
                            .await,
                    ),
                    (Err(error), _) | (_, Err(error)) => tool_error(error),
                }
            }
            ToolName::TaskResult => {
                let max_bytes = params.arguments.get("maxBytes").and_then(Value::as_i64);
                match (
                    require_task_id(&params.arguments),
                    TaskManagerHandle::from_env().await,
                ) {
                    (Ok(task_id), Ok(manager)) => {
                        tool_result(manager.result(task_id, max_bytes).await)
                    }
                    (Err(error), _) | (_, Err(error)) => tool_error(error),
                }
            }
            ToolName::TaskStop => {
                match (
                    require_task_id(&params.arguments),
                    TaskManagerHandle::from_env().await,
                ) {
                    (Ok(task_id), Ok(manager)) => tool_result(manager.stop(task_id).await),
                    (Err(error), _) | (_, Err(error)) => tool_error(error),
                }
            }
            ToolName::TaskRemove => {
                match (
                    require_task_id(&params.arguments),
                    TaskManagerHandle::from_env().await,
                ) {
                    (Ok(task_id), Ok(manager)) => tool_result(manager.remove(task_id).await),
                    (Err(error), _) | (_, Err(error)) => tool_error(error),
                }
            }
        },
        Err(error) => tool_error(error.to_string()),
    }
}

fn reject_unknown_arguments(name: ToolName, arguments: &Value) -> Result<(), String> {
    let allowed = match name {
        ToolName::ProvidersList => &[][..],
        ToolName::ProvidersCheck => &["smoke", "timeoutMs"][..],
        ToolName::TaskPreview | ToolName::TaskSpawn => &[
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
        ][..],
        ToolName::TaskList => &[][..],
        ToolName::TaskStatus | ToolName::TaskStop | ToolName::TaskRemove => &["taskId"][..],
        ToolName::TaskWait => &["taskId", "timeoutMs"][..],
        ToolName::TaskLogs => &["taskId", "maxBytes", "stdoutLine", "stderrLine"][..],
        ToolName::TaskResult => &["taskId", "maxBytes"][..],
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
        ToolName::ProvidersCheck => "providers_check",
        ToolName::TaskPreview => "task_preview",
        ToolName::TaskSpawn => "task_spawn",
        ToolName::TaskList => "task_list",
        ToolName::TaskStatus => "task_status",
        ToolName::TaskWait => "task_wait",
        ToolName::TaskLogs => "task_logs",
        ToolName::TaskResult => "task_result",
        ToolName::TaskStop => "task_stop",
        ToolName::TaskRemove => "task_remove",
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProvidersCheckInput {
    #[serde(default)]
    smoke: bool,
    timeout_ms: Option<i64>,
}

async fn providers_check(arguments: Value) -> Value {
    let input: ProvidersCheckInput =
        serde_json::from_value(arguments).unwrap_or(ProvidersCheckInput {
            smoke: false,
            timeout_ms: None,
        });
    let timeout_ms = input.timeout_ms.unwrap_or(5000).clamp(1, 60_000) as u64;
    let mut results = serde_json::Map::new();
    for provider in ProviderKind::ALL {
        let command = provider::version_command(provider);
        let output = run_probe(&command, provider, timeout_ms).await;
        let value = match output.status {
            Some(status) if status.success() && output.failure_category.is_none() => json!({
                "available": true,
                "command": command.command,
                "version": String::from_utf8_lossy(&output.stdout).trim(),
                "probe": if input.smoke { "version+smoke" } else { "version" },
                "startupVerified": false
            }),
            _ => json!({
                "available": false,
                "command": command.command,
                "probe": "version",
                "startupVerified": false,
                "error": probe_error_text(&output),
                "diagnostic": provider_diagnostic(provider, &command, &output, timeout_ms, false)
            }),
        };
        if input.smoke && value["available"].as_bool() == Some(true) {
            let smoke_value = match provider::smoke_command(
                provider,
                &default_cwd(),
                (timeout_ms / 1000).max(1) as i64,
            ) {
                Ok(smoke_command) => {
                    let mut output = run_probe(&smoke_command, provider, timeout_ms).await;
                    if provider == ProviderKind::Claude
                        && output
                            .status
                            .as_ref()
                            .is_some_and(|status| status.success())
                        && !claude_output_is_parseable(&output.stdout)
                    {
                        output.failure_category = Some("provider_output_error");
                    }
                    match output.status {
                        Some(status) if status.success() && output.failure_category.is_none() => {
                            let mut value = value;
                            value["startupVerified"] = json!(true);
                            value
                        }
                        _ => {
                            let mut value = value;
                            value["available"] = json!(false);
                            value["startupVerified"] = json!(false);
                            value["error"] = json!(probe_error_text(&output));
                            value["diagnostic"] = provider_diagnostic(
                                provider,
                                &smoke_command,
                                &output,
                                timeout_ms,
                                false,
                            );
                            value
                        }
                    }
                }
                Err(error) => {
                    let mut value = value;
                    value["available"] = json!(false);
                    value["error"] = json!(error);
                    value
                }
            };
            results.insert(provider.as_str().to_string(), smoke_value);
        } else {
            results.insert(provider.as_str().to_string(), value);
        }
    }
    json!({ "providers": results })
}

struct ProbeResult {
    status: Option<std::process::ExitStatus>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    failure_category: Option<&'static str>,
    error: Option<String>,
}

async fn run_probe(
    command: &provider::ProviderCommand,
    provider: ProviderKind,
    timeout_ms: u64,
) -> ProbeResult {
    let child = tokio::process::Command::new(&command.command)
        .args(&command.args)
        .env_clear()
        .envs(provider::provider_env(provider))
        .stdin(if command.stdin.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| ProbeResult {
            status: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
            failure_category: Some("provider_start_error"),
            error: Some(error.to_string()),
        });
    let mut child = match child {
        Ok(child) => child,
        Err(result) => return result,
    };
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
            let _ = child.start_kill();
            let status = child.wait().await.ok();
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
    }
}

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
) -> Value {
    let redactions = diagnostic_redactions(command);
    let mut diagnostic = json!({
        "failureCategory": output.failure_category.unwrap_or("provider_output_error"),
        "provider": provider.as_str(),
        "commandKind": command_kind(provider, command),
        "commandPath": command_path(provider, command),
        "startupVerified": startup_verified,
        "timeoutMs": timeout_ms,
        "exitCode": output.status.as_ref().and_then(|status| status.code()),
        "signal": signal_name(output.status.as_ref()),
        "stdoutExcerpt": excerpt(&output.stdout, &redactions),
        "stderrExcerpt": excerpt(&output.stderr, &redactions)
    });
    if provider == ProviderKind::Claude
        && command.command_kind.as_deref() == Some("claude-p")
        && env::var("CLAUDE_BIN").is_ok()
    {
        diagnostic["recommendation"] = json!(
            "Set CLAUDE_BIN without CLAUDE_P_BIN to use native claude -p, then retry providers_check with smoke: true"
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

fn command_kind(provider: ProviderKind, command: &provider::ProviderCommand) -> String {
    if provider != ProviderKind::Claude {
        return provider.as_str().to_string();
    }
    command
        .command_kind
        .as_deref()
        .unwrap_or("native-claude")
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
    Ok(json!({
        "command": command.command,
        "cwd": command.cwd,
        "timeoutSeconds": command.timeout_seconds,
        "args": args,
        "stdin": command.stdin.as_ref().map(|_| "<prompt redacted>"),
        "envKeys": env_keys
    }))
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
    let allowed_root = env::var("AGENT_BRIDGE_ALLOWED_ROOT")
        .ok()
        .map(PathBuf::from);
    let root =
        allowed_root.unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let real_cwd = cwd.canonicalize().map_err(|error| error.to_string())?;
    let real_root = root.canonicalize().map_err(|error| error.to_string())?;
    if !is_inside(&real_cwd, &real_root) {
        return Err(format!(
            "cwd is outside allowed root: {}",
            real_root.display()
        ));
    }
    Ok(real_cwd.display().to_string())
}

fn is_inside(candidate: &Path, root: &Path) -> bool {
    candidate == root || candidate.strip_prefix(root).is_ok()
}

fn tool_json(value: Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap() }],
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

fn require_task_id(arguments: &Value) -> Result<String, String> {
    arguments
        .get("taskId")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "taskId is required".to_string())
}
