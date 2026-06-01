use crate::domain::{Isolation, ProviderKind, TimeoutSeconds, WorktreeName};
use crate::guidance;
use crate::mcp::{JsonRpcId, JsonRpcRequest, JsonRpcResponse};
use crate::provider::{self, ProviderTask};
use crate::task::TaskManagerHandle;
use crate::tools::{TaskPreviewInput, ToolCallParams, ToolName, tool_definitions};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};
use std::env;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinSet;
use tokio::time::{Duration, Instant, timeout};

const PROTOCOL_VERSION: &str = "2024-11-05";
const MAX_PROMPT_BYTES: usize = 100 * 1024;
const VERSION_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_AGGREGATE_TIMEOUT_MS: u64 = 110_000;
const MAX_AGGREGATE_TIMEOUT_MS: i64 = 120_000;
const MAX_PROVIDER_TIMEOUT_MS: i64 = 90_000;
const CHILD_SHUTDOWN_GRACE: Duration = Duration::from_millis(500);

pub async fn handle_request(request: JsonRpcRequest) -> Option<JsonRpcResponse> {
    request.id.as_ref()?;
    let id = request.id.clone().unwrap_or(JsonRpcId::Null);
    let response = match request.method.as_str() {
        "initialize" => JsonRpcResponse::result(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {}, "prompts": {}, "resources": {} },
                "serverInfo": { "name": "agent-bridge-mcp", "version": "0.1.0" }
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
    match parsed {
        Ok(params) => match params.name {
            name if reject_unknown_arguments(name, &params.arguments).is_err() => {
                tool_error(reject_unknown_arguments(name, &params.arguments).unwrap_err())
            }
            ToolName::ProvidersList => tool_json(json!({ "providers": provider::capabilities() })),
            ToolName::ProvidersCheck => tool_result(providers_check(params.arguments).await),
            ToolName::Doctor => tool_result(doctor(params.arguments).await),
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
        ToolName::ProvidersCheck => &[
            "smoke",
            "timeoutMs",
            "providers",
            "aggregateTimeoutMs",
            "providerTimeoutMs",
        ][..],
        ToolName::Doctor => &[
            "smoke",
            "providers",
            "aggregateTimeoutMs",
            "providerTimeoutMs",
            "cwd",
        ][..],
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
        ToolName::Doctor => "doctor",
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
    providers: Option<Vec<ProviderKind>>,
    aggregate_timeout_ms: Option<i64>,
    provider_timeout_ms: Option<BTreeMap<String, i64>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DoctorInput {
    #[serde(default)]
    smoke: bool,
    providers: Option<Vec<ProviderKind>>,
    aggregate_timeout_ms: Option<i64>,
    provider_timeout_ms: Option<BTreeMap<String, i64>>,
    cwd: Option<String>,
}

async fn doctor(arguments: Value) -> Result<Value, String> {
    let input: DoctorInput =
        serde_json::from_value(arguments).map_err(|error| error.to_string())?;
    let workspace = doctor_workspace(input.cwd.as_deref());
    let state = doctor_state();
    let claude_host_runner = doctor_claude_host_runner().await;
    let provider_report = providers_check(doctor_provider_arguments(&input)).await?;
    let providers = provider_report
        .get("providers")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let provider_status = providers_status(&providers);
    let recommendations = doctor_recommendations(
        workspace["status"].as_str().unwrap_or("ok"),
        state["status"].as_str().unwrap_or("ok"),
        provider_status,
        claude_host_runner["status"].as_str().unwrap_or("ok"),
    );
    let summary_status = aggregate_status([
        workspace["status"].as_str().unwrap_or("ok"),
        state["status"].as_str().unwrap_or("ok"),
        provider_status,
        claude_host_runner["status"].as_str().unwrap_or("ok"),
    ]);

    Ok(json!({
        "summary": {
            "status": summary_status,
            "checkedAt": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        },
        "server": {
            "name": "agent-bridge-mcp",
            "version": "0.1.0",
            "protocolVersion": PROTOCOL_VERSION,
            "environment": doctor_environment()
        },
        "workspace": workspace,
        "state": state,
        "providers": providers,
        "claudeHostRunner": claude_host_runner,
        "recommendations": recommendations
    }))
}

fn doctor_provider_arguments(input: &DoctorInput) -> Value {
    let mut arguments = serde_json::Map::new();
    arguments.insert("smoke".to_string(), json!(input.smoke));
    if let Some(providers) = input.providers.as_ref() {
        arguments.insert("providers".to_string(), json!(providers));
    }
    if let Some(aggregate_timeout_ms) = input.aggregate_timeout_ms {
        arguments.insert(
            "aggregateTimeoutMs".to_string(),
            json!(aggregate_timeout_ms),
        );
    }
    if let Some(provider_timeout_ms) = input.provider_timeout_ms.as_ref() {
        arguments.insert("providerTimeoutMs".to_string(), json!(provider_timeout_ms));
    }
    Value::Object(arguments)
}

fn doctor_workspace(cwd: Option<&str>) -> Value {
    let roots = match doctor_configured_workspace_roots() {
        Ok(roots) => roots,
        Err(error) => {
            return json!({
                "status": "error",
                "error": error
            });
        }
    };
    let cwd_report = match safe_cwd(cwd) {
        Ok(path) => json!({
            "status": "ok",
            "path": path,
            "insideConfiguredWorkspace": true
        }),
        Err(error) => json!({
            "status": "error",
            "error": error,
            "insideConfiguredWorkspace": false
        }),
    };
    let status = if cwd_report["status"] == "error" {
        "error"
    } else {
        "ok"
    };
    json!({
        "status": status,
        "roots": roots
            .iter()
            .map(|root| root.display().to_string())
            .collect::<Vec<_>>(),
        "cwd": cwd_report
    })
}

fn doctor_configured_workspace_roots() -> Result<Vec<PathBuf>, String> {
    let Some(value) = env::var_os("AGENT_BRIDGE_WORKSPACES") else {
        return Err(
            "AGENT_BRIDGE_WORKSPACES is required for doctor workspace diagnostics".to_string(),
        );
    };
    let roots: Vec<PathBuf> = env::split_paths(&value)
        .filter(|path| !path.as_os_str().is_empty())
        .collect();
    if roots.is_empty() {
        return Err(
            "AGENT_BRIDGE_WORKSPACES is required for doctor workspace diagnostics".to_string(),
        );
    }
    roots
        .into_iter()
        .map(|root| root.canonicalize().map_err(|error| error.to_string()))
        .collect()
}

fn doctor_environment() -> Value {
    const KEYS: &[&str] = &[
        "AGENT_BRIDGE_WORKSPACES",
        "AGENT_BRIDGE_STATE_DIR",
        "AGENT_BRIDGE_CLAUDE_HOST_SOCKET",
        "CODEX_BIN",
        "CURSOR_AGENT_BIN",
        "PI_BIN",
        "CLAUDE_BIN",
        "CLAUDE_P_BIN",
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "ANTHROPIC_BASE_URL",
    ];
    let mut environment = serde_json::Map::new();
    for key in KEYS {
        let present = env::var_os(key).is_some_and(|value| !value.is_empty());
        let mut entry = json!({ "present": present });
        if present && is_sensitive_env_key(key) {
            entry["value"] = json!("<redacted>");
        }
        environment.insert((*key).to_string(), entry);
    }
    Value::Object(environment)
}

fn is_sensitive_env_key(key: &str) -> bool {
    let key = key.to_ascii_uppercase();
    ["TOKEN", "API_KEY", "OAUTH", "AUTH", "PASSWORD", "SECRET"]
        .iter()
        .any(|needle| key.contains(needle))
}

fn doctor_state() -> Value {
    let path = env::var("AGENT_BRIDGE_STATE_DIR")
        .map(|value| expand_home(&value))
        .unwrap_or_else(|_| expand_home("~/.agent-bridge"));
    if let Err(error) = std::fs::create_dir_all(&path) {
        return json!({
            "status": "error",
            "path": path.display().to_string(),
            "exists": false,
            "error": error.to_string()
        });
    }
    match std::fs::metadata(&path) {
        Ok(metadata) if metadata.is_dir() => match doctor_registry_status(&path) {
            Ok(()) => json!({
                "status": "ok",
                "path": path.display().to_string(),
                "exists": true
            }),
            Err(error) => json!({
                "status": "error",
                "path": path.display().to_string(),
                "exists": true,
                "error": error
            }),
        },
        Ok(_) => json!({
            "status": "error",
            "path": path.display().to_string(),
            "exists": true,
            "error": "state path is not a directory"
        }),
        Err(error) => json!({
            "status": "error",
            "path": path.display().to_string(),
            "exists": false,
            "error": error.to_string()
        }),
    }
}

fn doctor_registry_status(state_dir: &Path) -> Result<(), String> {
    let registry_path = state_dir.join("registry.json");
    match std::fs::read_to_string(&registry_path) {
        Ok(contents) => serde_json::from_str::<Value>(&contents)
            .map(|_| ())
            .map_err(|error| format!("registry parse error: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("registry read error: {error}")),
    }
}

async fn doctor_claude_host_runner() -> Value {
    let Some(socket_path) = crate::claude_host::socket_path_from_env() else {
        return json!({
            "status": "not_configured",
            "configured": false,
            "launchStrategy": "direct"
        });
    };
    let started = Instant::now();
    match timeout(
        Duration::from_millis(1_000),
        crate::claude_host::ping(&socket_path),
    )
    .await
    {
        Ok(Ok(response)) => doctor_claude_host_runner_response(
            &socket_path,
            started.elapsed().as_millis() as u64,
            response,
        ),
        Ok(Err(error)) => json!({
            "status": "error",
            "configured": true,
            "launchStrategy": "host_runner",
            "socketPath": socket_path.display().to_string(),
            "pingDurationMs": started.elapsed().as_millis() as u64,
            "error": error
        }),
        Err(_) => json!({
            "status": "error",
            "configured": true,
            "launchStrategy": "host_runner",
            "socketPath": socket_path.display().to_string(),
            "pingDurationMs": started.elapsed().as_millis() as u64,
            "error": "host runner ping timed out after 1000ms"
        }),
    }
}

fn doctor_claude_host_runner_response(
    socket_path: &Path,
    ping_duration_ms: u64,
    response: crate::claude_host::HostResponse,
) -> Value {
    if response.ok {
        let mut report = json!({
            "status": "ok",
            "configured": true,
            "launchStrategy": "host_runner",
            "socketPath": socket_path.display().to_string(),
            "pingDurationMs": ping_duration_ms
        });
        if let Some(crate::claude_host::HostResult::Pong {
            protocol_version,
            workspace_policy_id,
            ready,
        }) = response.result
        {
            report["protocolVersion"] = json!(protocol_version);
            report["workspacePolicyId"] = json!(workspace_policy_id);
            report["ready"] = json!(ready);
        }
        return report;
    }
    json!({
        "status": "error",
        "configured": true,
        "launchStrategy": "host_runner",
        "socketPath": socket_path.display().to_string(),
        "pingDurationMs": ping_duration_ms,
        "errorCode": response.error.as_ref().map(|error| error.code.as_str()).unwrap_or("host_runner_error"),
        "error": response.error.map(|error| error.message).unwrap_or_else(|| "host runner ping failed".to_string())
    })
}

fn providers_status(providers: &Value) -> &'static str {
    let Some(providers) = providers.as_object() else {
        return "error";
    };
    if providers.values().any(|provider| {
        provider
            .get("available")
            .and_then(Value::as_bool)
            .is_some_and(|available| !available)
    }) {
        "warning"
    } else {
        "ok"
    }
}

fn doctor_recommendations(
    workspace_status: &str,
    state_status: &str,
    provider_status: &str,
    host_status: &str,
) -> Value {
    let mut recommendations = Vec::new();
    if workspace_status == "error" {
        recommendations.push(json!({
            "severity": "error",
            "message": "Set AGENT_BRIDGE_WORKSPACES or pass a cwd inside a configured workspace."
        }));
    }
    if state_status == "error" {
        recommendations.push(json!({
            "severity": "error",
            "message": "Fix AGENT_BRIDGE_STATE_DIR so Agent Bridge can read and write task state."
        }));
    } else if state_status == "warning" {
        recommendations.push(json!({
            "severity": "warning",
            "message": "Create AGENT_BRIDGE_STATE_DIR before spawning delegated tasks."
        }));
    }
    if host_status == "error" {
        recommendations.push(json!({
            "severity": "warning",
            "message": "Restart or reconfigure the Claude host runner with matching AGENT_BRIDGE_WORKSPACES, then rerun doctor."
        }));
    }
    if provider_status == "warning" {
        recommendations.push(json!({
            "severity": "warning",
            "message": "Install or configure unavailable providers, or pass providers to focus doctor output."
        }));
    }
    Value::Array(recommendations)
}

fn aggregate_status<'a>(statuses: impl IntoIterator<Item = &'a str>) -> &'static str {
    let mut aggregate = "ok";
    for status in statuses {
        match status {
            "error" => return "error",
            "warning" if aggregate == "ok" => aggregate = "warning",
            _ => {}
        }
    }
    aggregate
}

async fn providers_check(arguments: Value) -> Result<Value, String> {
    let input: ProvidersCheckInput =
        serde_json::from_value(arguments).map_err(|error| error.to_string())?;
    let selected = selected_providers(input.providers.as_deref())?;
    validate_provider_budgets(&input)?;
    let aggregate_timeout_ms = aggregate_timeout_ms(input.aggregate_timeout_ms)?;
    let mut results = serde_json::Map::new();
    let mut smoke_candidates = Vec::new();
    for provider in selected.iter().copied() {
        let command = provider::version_command(provider);
        let output = run_probe(&command, provider, VERSION_TIMEOUT_MS, "version").await;
        let value = match output.status {
            Some(status) if status.success() && output.failure_category.is_none() => json!({
                "available": true,
                "command": command.command,
                "version": String::from_utf8_lossy(&output.stdout).trim(),
                "probe": if input.smoke { "version+smoke" } else { "version" },
                "startupVerified": false,
                "versionDurationMs": output.duration_ms
            }),
            _ => json!({
                "available": false,
                "command": command.command,
                "probe": "version",
                "startupVerified": false,
                "error": probe_error_text(&output),
                "versionDurationMs": output.duration_ms,
                "diagnostic": provider_diagnostic(provider, &command, &output, VERSION_TIMEOUT_MS, false, "version")
            }),
        };
        if input.smoke && value["available"].as_bool() == Some(true) {
            smoke_candidates.push((provider, value));
        } else {
            results.insert(provider.as_str().to_string(), value);
        }
    }
    if input.smoke {
        let smoked = run_smoke_checks(smoke_candidates, &input, aggregate_timeout_ms).await;
        for (provider, value) in smoked {
            results.insert(provider.as_str().to_string(), value);
        }
    }
    Ok(json!({ "providers": results }))
}

fn selected_providers(input: Option<&[ProviderKind]>) -> Result<Vec<ProviderKind>, String> {
    let Some(input) = input else {
        return Ok(ProviderKind::ALL.to_vec());
    };
    if input.is_empty() {
        return Err("providers must select at least one provider".to_string());
    }
    let mut selected = Vec::new();
    for provider in input {
        if !selected.contains(provider) {
            selected.push(*provider);
        }
    }
    Ok(selected)
}

fn validate_provider_budgets(input: &ProvidersCheckInput) -> Result<(), String> {
    let Some(provider_timeout_ms) = input.provider_timeout_ms.as_ref() else {
        return Ok(());
    };
    for (provider, value) in provider_timeout_ms {
        provider
            .parse::<ProviderKind>()
            .map_err(|error| format!("providerTimeoutMs.{provider}: {error}"))?;
        validate_timeout_range(
            *value,
            MAX_PROVIDER_TIMEOUT_MS,
            &format!("providerTimeoutMs.{provider}"),
        )?;
    }
    Ok(())
}

fn aggregate_timeout_ms(value: Option<i64>) -> Result<u64, String> {
    match value {
        Some(value) => {
            validate_timeout_range(value, MAX_AGGREGATE_TIMEOUT_MS, "aggregateTimeoutMs")
        }
        None => Ok(DEFAULT_AGGREGATE_TIMEOUT_MS),
    }
}

fn validate_timeout_range(value: i64, max: i64, field: &str) -> Result<u64, String> {
    if !(1..=max).contains(&value) {
        return Err(format!("{field} must be an integer from 1 through {max}"));
    }
    Ok(value as u64)
}

fn provider_smoke_timeout_ms(provider: ProviderKind, input: &ProvidersCheckInput) -> u64 {
    input
        .provider_timeout_ms
        .as_ref()
        .and_then(|timeouts| timeouts.get(provider.as_str()))
        .copied()
        .or(input.timeout_ms)
        .map(|value| value.clamp(1, MAX_PROVIDER_TIMEOUT_MS) as u64)
        .unwrap_or_else(|| default_provider_smoke_timeout_ms(provider))
}

fn default_provider_smoke_timeout_ms(provider: ProviderKind) -> u64 {
    match provider {
        ProviderKind::Codex => 20_000,
        ProviderKind::Claude => 30_000,
        ProviderKind::Kimi => 45_000,
        ProviderKind::Cursor => 60_000,
    }
}

fn smoke_concurrency() -> usize {
    env::var("AGENT_BRIDGE_SMOKE_CONCURRENCY")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .map(|value| value.min(4))
        .unwrap_or(2)
}

async fn run_smoke_checks(
    candidates: Vec<(ProviderKind, Value)>,
    input: &ProvidersCheckInput,
    aggregate_timeout_ms: u64,
) -> Vec<(ProviderKind, Value)> {
    let order: Vec<ProviderKind> = candidates.iter().map(|(provider, _)| *provider).collect();
    let deadline = Instant::now() + Duration::from_millis(aggregate_timeout_ms);
    let mut pending: VecDeque<_> = candidates.into();
    let mut running = JoinSet::new();
    let mut results = Vec::new();
    let concurrency = smoke_concurrency();
    loop {
        while running.len() < concurrency && !pending.is_empty() {
            let remaining_ms = deadline
                .checked_duration_since(Instant::now())
                .map(|duration| duration.as_millis() as u64)
                .unwrap_or(0);
            if remaining_ms == 0 {
                break;
            }
            let (provider, base_value) = pending.pop_front().unwrap();
            let provider_timeout_ms = provider_smoke_timeout_ms(provider, input);
            let timeout_ms = provider_timeout_ms.min(remaining_ms);
            running
                .spawn(async move { smoke_one_provider(provider, base_value, timeout_ms).await });
        }
        if running.is_empty() {
            break;
        }
        match running.join_next().await {
            Some(Ok(result)) => results.push(result),
            Some(Err(error)) => eprintln!("[agent-bridge] smoke task join error: {error}"),
            None => break,
        }
    }
    for (provider, mut value) in pending {
        value["available"] = json!(false);
        value["startupVerified"] = json!(false);
        value["error"] =
            json!("aggregate provider readiness timeout expired before smoke probe started");
        value["diagnostic"] = json!({
            "failureCategory": "provider_timeout",
            "provider": provider.as_str(),
            "startupVerified": false,
            "timeoutMs": aggregate_timeout_ms,
            "phase": "smoke"
        });
        results.push((provider, value));
    }
    results.sort_by_key(|(provider, _)| {
        order
            .iter()
            .position(|candidate| candidate == provider)
            .unwrap_or(usize::MAX)
    });
    results
}

async fn smoke_one_provider(
    provider: ProviderKind,
    base_value: Value,
    timeout_ms: u64,
) -> (ProviderKind, Value) {
    let smoke_value = match provider::smoke_command(
        provider,
        &default_cwd(),
        (timeout_ms / 1000).max(1) as i64,
    ) {
        Ok((smoke_command, strategy)) => {
            let mut output = run_probe(&smoke_command, provider, timeout_ms, "smoke").await;
            if output
                .status
                .as_ref()
                .is_some_and(|status| status.success())
                && !smoke_output_is_accepted(provider, &output.stdout)
            {
                output.failure_category = Some("provider_output_error");
                output.error =
                    Some("provider smoke output did not contain expected token".to_string());
            }
            match output.status {
                Some(status) if status.success() && output.failure_category.is_none() => {
                    let mut value = base_value;
                    value["startupVerified"] = json!(true);
                    value["smokeDurationMs"] = json!(output.duration_ms);
                    value["smokePromptStrategy"] = json!(strategy);
                    if provider == ProviderKind::Claude
                        && crate::claude_host::socket_path_from_env().is_some()
                    {
                        value["launchStrategy"] = json!("host_runner");
                    }
                    value
                }
                _ => {
                    let mut value = base_value;
                    value["available"] = json!(false);
                    value["startupVerified"] = json!(false);
                    value["smokeDurationMs"] = json!(output.duration_ms);
                    value["smokePromptStrategy"] = json!(strategy);
                    if provider == ProviderKind::Claude
                        && crate::claude_host::socket_path_from_env().is_some()
                    {
                        value["launchStrategy"] = json!("host_runner");
                    }
                    value["error"] = json!(probe_error_text(&output));
                    value["diagnostic"] = provider_diagnostic(
                        provider,
                        &smoke_command,
                        &output,
                        timeout_ms,
                        false,
                        "smoke",
                    );
                    value
                }
            }
        }
        Err(error) => {
            let mut value = base_value;
            value["available"] = json!(false);
            value["error"] = json!(error);
            value
        }
    };
    (provider, smoke_value)
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
        Some(crate::claude_host::HostResult::Run {
            exit_code,
            signal,
            stdout,
            stderr,
            failure_category,
            duration_ms,
            ..
        }) => ProbeResult {
            status: host_exit_status(exit_code, signal.as_deref()),
            stdout: stdout.into_bytes(),
            stderr: stderr.into_bytes(),
            failure_category: failure_category.as_deref().map(|category| match category {
                "provider_timeout" => "provider_timeout",
                "provider_exit_error" => "provider_exit_error",
                _ => "provider_output_error",
            }),
            error: failure_category,
            duration_ms,
        },
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
        "launchStrategy": if provider == ProviderKind::Claude && crate::claude_host::socket_path_from_env().is_some() { "host_runner" } else { "direct" },
        "startupVerified": startup_verified,
        "timeoutMs": timeout_ms,
        "elapsedMs": output.duration_ms,
        "phase": phase,
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
        ProviderKind::Cursor | ProviderKind::Kimi => text.contains(provider::PROVIDER_SMOKE_TOKEN),
    }
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
    let mut preview = json!({
        "command": command.command,
        "cwd": command.cwd,
        "timeoutSeconds": command.timeout_seconds,
        "args": args,
        "stdin": command.stdin.as_ref().map(|_| "<prompt redacted>"),
        "envKeys": env_keys
    });
    if input.provider == ProviderKind::Claude
        && crate::claude_host::socket_path_from_env().is_some()
    {
        preview["launchStrategy"] = json!("host_runner");
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

#[cfg(test)]
mod tests {
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
        let recommendations = doctor_recommendations("ok", "ok", "ok", "error");
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
}
