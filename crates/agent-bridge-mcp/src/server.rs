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
use std::sync::{Mutex, OnceLock};
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
const MAX_CLIENT_CONFIG_BYTES: u64 = 1024 * 1024;
const MAX_BINARY_FINGERPRINT_BYTES: u64 = 16 * 1024 * 1024;
const TASKS_EXTENSION_ID: &str = "io.modelcontextprotocol/tasks";

static TASK_EXTENSION_READINESS: OnceLock<Mutex<TaskExtensionReadiness>> = OnceLock::new();

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
    match parsed {
        Ok(params) => match params.name {
            name if reject_unknown_arguments(name, &params.arguments).is_err() => {
                tool_error(reject_unknown_arguments(name, &params.arguments).unwrap_err())
            }
            ToolName::ProvidersList => tool_json(json!({ "providers": provider::capabilities() })),
            ToolName::ProvidersCheck => tool_result(providers_check(params.arguments).await),
            ToolName::Doctor => tool_result(doctor(params.arguments).await),
            ToolName::AgentPreview => match task_preview(params.arguments) {
                Ok(payload) => tool_json(payload),
                Err(error) => tool_error(error),
            },
            ToolName::AgentSpawn => match TaskManagerHandle::from_env().await {
                Ok(manager) => tool_result(manager.spawn(params.arguments).await),
                Err(error) => tool_error(error),
            },
            ToolName::AgentList => match TaskManagerHandle::from_env().await {
                Ok(manager) => tool_result(agent_list(manager, params.arguments).await),
                Err(error) => tool_error(error),
            },
            ToolName::AgentStatus => {
                match (
                    require_task_id(&params.arguments),
                    TaskManagerHandle::from_env().await,
                ) {
                    (Ok(task_id), Ok(manager)) => tool_result(manager.status(task_id).await),
                    (Err(error), _) | (_, Err(error)) => tool_error(error),
                }
            }
            ToolName::AgentWait => {
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
            ToolName::AgentLogs => {
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
            ToolName::AgentTranscript => {
                let cursor = params.arguments.get("cursor").and_then(Value::as_u64);
                let limit = params.arguments.get("limit").and_then(Value::as_u64);
                match (
                    require_task_id(&params.arguments),
                    TaskManagerHandle::from_env().await,
                ) {
                    (Ok(task_id), Ok(manager)) => {
                        tool_result(manager.transcript(task_id, cursor, limit).await)
                    }
                    (Err(error), _) | (_, Err(error)) => tool_error(error),
                }
            }
            ToolName::AgentObserve => {
                let cursor = params.arguments.get("cursor").and_then(Value::as_u64);
                let limit = params.arguments.get("limit").and_then(Value::as_u64);
                let timeout_ms = params.arguments.get("timeoutMs").and_then(Value::as_i64);
                match (
                    require_task_id(&params.arguments),
                    TaskManagerHandle::from_env().await,
                ) {
                    (Ok(task_id), Ok(manager)) => {
                        tool_result(manager.observe(task_id, cursor, limit, timeout_ms).await)
                    }
                    (Err(error), _) | (_, Err(error)) => tool_error(error),
                }
            }
            ToolName::AgentResult => {
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
            ToolName::AgentStop => {
                match (
                    require_task_id(&params.arguments),
                    TaskManagerHandle::from_env().await,
                ) {
                    (Ok(task_id), Ok(manager)) => tool_result(manager.stop(task_id).await),
                    (Err(error), _) | (_, Err(error)) => tool_error(error),
                }
            }
            ToolName::AgentRemove => {
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
        ToolName::AgentPreview | ToolName::AgentSpawn => &[
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
        ][..],
        ToolName::AgentList => &[
            "status",
            "provider",
            "mode",
            "cwd",
            "titleContains",
            "limit",
        ][..],
        ToolName::AgentStatus | ToolName::AgentStop | ToolName::AgentRemove => &["taskId"][..],
        ToolName::AgentWait => &["taskId", "timeoutMs"][..],
        ToolName::AgentLogs => &["taskId", "maxBytes", "stdoutLine", "stderrLine"][..],
        ToolName::AgentTranscript => &["taskId", "cursor", "limit"][..],
        ToolName::AgentObserve => &["taskId", "cursor", "limit", "timeoutMs"][..],
        ToolName::AgentResult => &["taskId", "maxBytes"][..],
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
        ToolName::AgentPreview => "agent_preview",
        ToolName::AgentSpawn => "agent_spawn",
        ToolName::AgentList => "agent_list",
        ToolName::AgentStatus => "agent_status",
        ToolName::AgentWait => "agent_wait",
        ToolName::AgentLogs => "agent_logs",
        ToolName::AgentTranscript => "agent_transcript",
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
    let binary = doctor_binary(input.cwd.as_deref());
    let clients = doctor_clients();
    let task_extension_readiness = doctor_task_extension_readiness();
    let claude_host_runner = doctor_claude_host_runner().await;
    let provider_report = providers_check(doctor_provider_arguments(&input)).await?;
    let providers = provider_report
        .get("providers")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let provider_status = providers_status(&providers);
    let launch_readiness = doctor_launch_readiness(&providers, input.providers.as_deref());
    let recommendations = doctor_recommendations(
        workspace["status"].as_str().unwrap_or("ok"),
        state["status"].as_str().unwrap_or("ok"),
        provider_status,
        claude_host_runner["status"].as_str().unwrap_or("ok"),
        &launch_readiness,
        &clients,
        &binary,
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
        "binary": binary,
        "clients": clients,
        "taskExtensionReadiness": task_extension_readiness,
        "providers": providers,
        "launchReadiness": launch_readiness,
        "claudeHostRunner": claude_host_runner,
        "recommendations": recommendations
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TaskExtensionClassification {
    Unavailable,
    Unknown,
    LegacyOnly,
    ExtensionCapable,
    Unsupported,
}

impl TaskExtensionClassification {
    fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Unknown => "unknown",
            Self::LegacyOnly => "legacy_only",
            Self::ExtensionCapable => "extension_capable",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug, Clone)]
struct TaskExtensionReadiness {
    classification: TaskExtensionClassification,
    source: &'static str,
    observed_extension_identifiers: Vec<String>,
    legacy_indicators: Vec<String>,
    unknown_indicators: Vec<String>,
    checked_at: String,
}

impl Default for TaskExtensionReadiness {
    fn default() -> Self {
        Self {
            classification: TaskExtensionClassification::Unavailable,
            source: "none",
            observed_extension_identifiers: Vec::new(),
            legacy_indicators: Vec::new(),
            unknown_indicators: Vec::new(),
            checked_at: checked_at_iso(),
        }
    }
}

impl TaskExtensionReadiness {
    fn recommended_next_step(&self) -> &'static str {
        match self.classification {
            TaskExtensionClassification::ExtensionCapable => {
                "Use Agent Bridge agent_* tools; protocol task support is not advertised yet. Extension metadata can inform future implementation work."
            }
            TaskExtensionClassification::LegacyOnly => {
                "Use Agent Bridge agent_* tools; legacy task metadata does not unblock current extension-based task support."
            }
            TaskExtensionClassification::Unknown => {
                "Use Agent Bridge agent_* tools; inspect unknown task-like metadata before designing protocol task support."
            }
            TaskExtensionClassification::Unsupported => {
                "Use Agent Bridge agent_* tools; requested protocol task behavior is not implemented or advertised."
            }
            TaskExtensionClassification::Unavailable => {
                "Use Agent Bridge agent_* tools; no MCP task-extension metadata has been observed."
            }
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "classification": self.classification.as_str(),
            "serverAdvertisesTasks": false,
            "source": self.source,
            "observedExtensionIdentifiers": self.observed_extension_identifiers,
            "legacyIndicators": self.legacy_indicators,
            "unknownIndicators": self.unknown_indicators,
            "recommendedNextStep": self.recommended_next_step(),
            "checkedAt": self.checked_at
        })
    }
}

#[derive(Debug, Default)]
struct TaskExtensionProbe {
    observed_extension_identifiers: Vec<String>,
    legacy_indicators: Vec<String>,
    unknown_indicators: Vec<String>,
    unsupported_indicators: Vec<String>,
}

impl TaskExtensionProbe {
    fn classification(&self) -> TaskExtensionClassification {
        if !self.unsupported_indicators.is_empty() {
            TaskExtensionClassification::Unsupported
        } else if !self.observed_extension_identifiers.is_empty() {
            TaskExtensionClassification::ExtensionCapable
        } else if !self.legacy_indicators.is_empty() {
            TaskExtensionClassification::LegacyOnly
        } else if !self.unknown_indicators.is_empty() {
            TaskExtensionClassification::Unknown
        } else {
            TaskExtensionClassification::Unavailable
        }
    }

    fn into_readiness(mut self, source: &'static str) -> TaskExtensionReadiness {
        sort_dedup_bound(&mut self.observed_extension_identifiers);
        sort_dedup_bound(&mut self.legacy_indicators);
        sort_dedup_bound(&mut self.unknown_indicators);
        sort_dedup_bound(&mut self.unsupported_indicators);
        let classification = self.classification();
        let mut unknown_indicators = self.unknown_indicators;
        unknown_indicators.extend(self.unsupported_indicators);
        sort_dedup_bound(&mut unknown_indicators);
        TaskExtensionReadiness {
            classification,
            source,
            observed_extension_identifiers: self.observed_extension_identifiers,
            legacy_indicators: self.legacy_indicators,
            unknown_indicators,
            checked_at: checked_at_iso(),
        }
    }
}

fn task_extension_readiness_store() -> &'static Mutex<TaskExtensionReadiness> {
    TASK_EXTENSION_READINESS.get_or_init(|| Mutex::new(TaskExtensionReadiness::default()))
}

fn observe_task_extension_metadata(request: &JsonRpcRequest) {
    let Some((metadata, source)) = task_extension_metadata_source(request) else {
        return;
    };
    let readiness = classify_task_extension_metadata(metadata, source);
    *task_extension_readiness_store().lock().unwrap() = readiness;
}

fn task_extension_metadata_source(request: &JsonRpcRequest) -> Option<(&Value, &'static str)> {
    match request.method.as_str() {
        "initialize" => request.params.as_ref().map(|params| (params, "initialize")),
        "tools/call" => request
            .params
            .as_ref()
            .and_then(|params| params.get("_meta"))
            .map(|meta| (meta, "request_meta")),
        _ => request
            .params
            .as_ref()
            .and_then(|params| params.get("_meta"))
            .map(|meta| (meta, "request_meta")),
    }
}

fn classify_task_extension_metadata(
    metadata: &Value,
    source: &'static str,
) -> TaskExtensionReadiness {
    let mut probe = TaskExtensionProbe::default();
    collect_task_extension_indicators(metadata, "", &mut probe);
    probe.into_readiness(source)
}

fn collect_task_extension_indicators(value: &Value, path: &str, probe: &mut TaskExtensionProbe) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                inspect_task_metadata_key(key, &child_path, value, probe);
                collect_task_extension_indicators(value, &child_path, probe);
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                let child_path = if path.is_empty() {
                    format!("[{index}]")
                } else {
                    format!("{path}[{index}]")
                };
                if let Some(identifier) = value
                    .get("id")
                    .or_else(|| value.get("name"))
                    .and_then(Value::as_str)
                {
                    inspect_task_metadata_identifier(identifier, &child_path, probe);
                }
                collect_task_extension_indicators(value, &child_path, probe);
            }
        }
        Value::String(value) => inspect_task_metadata_identifier(value, path, probe),
        _ => {}
    }
}

fn inspect_task_metadata_key(key: &str, path: &str, value: &Value, probe: &mut TaskExtensionProbe) {
    if key == TASKS_EXTENSION_ID {
        probe
            .observed_extension_identifiers
            .push(TASKS_EXTENSION_ID.to_string());
        return;
    }
    if key == "tasks" && path.contains("capabilities") {
        probe.legacy_indicators.push(bound_indicator(path));
        return;
    }
    let lower = key.to_ascii_lowercase();
    if lower.contains("unsupported") && lower.contains("task") {
        probe.unsupported_indicators.push(bound_indicator(path));
    } else if (lower.contains("require") || lower.contains("request")) && lower.contains("task") {
        if value.as_bool() == Some(true) {
            probe.unsupported_indicators.push(bound_indicator(path));
        }
    } else if lower.contains("task") {
        probe.unknown_indicators.push(bound_indicator(path));
    }
}

fn inspect_task_metadata_identifier(identifier: &str, path: &str, probe: &mut TaskExtensionProbe) {
    if identifier == TASKS_EXTENSION_ID {
        probe
            .observed_extension_identifiers
            .push(TASKS_EXTENSION_ID.to_string());
        return;
    }
    let lower = identifier.to_ascii_lowercase();
    if lower.contains("2025-11-25") && lower.contains("task") {
        probe.legacy_indicators.push(bound_indicator(path));
    } else if lower.contains("task") {
        probe.unknown_indicators.push(bound_indicator(path));
    }
}

fn bound_indicator(value: &str) -> String {
    const MAX_INDICATOR_CHARS: usize = 120;
    value.chars().take(MAX_INDICATOR_CHARS).collect()
}

fn sort_dedup_bound(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
    values.truncate(16);
}

fn doctor_task_extension_readiness() -> Value {
    task_extension_readiness_store().lock().unwrap().to_json()
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
        "AGENT_BRIDGE_INSTALLED_BIN",
        "AGENT_BRIDGE_RELEASE_BIN",
        "CODEX_BIN",
        "CURSOR_AGENT_BIN",
        "PI_BIN",
        "CLAUDE_BIN",
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

#[derive(Debug, Clone)]
struct BinaryTarget {
    path: PathBuf,
    exists: bool,
    readable: bool,
    size_bytes: Option<u64>,
    modified_at: Option<String>,
    fingerprint: Option<String>,
    fingerprint_status: &'static str,
    error: Option<String>,
}

impl BinaryTarget {
    fn inspect(path: PathBuf) -> Self {
        match std::fs::metadata(&path) {
            Ok(metadata) if metadata.is_file() => {
                let size_bytes = metadata.len();
                let modified_at = metadata.modified().ok().map(|time| {
                    chrono::DateTime::<chrono::Utc>::from(time)
                        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                });
                let (fingerprint, fingerprint_status, error) = fingerprint_file(&path, size_bytes);
                Self {
                    path,
                    exists: true,
                    readable: fingerprint_status != "error",
                    size_bytes: Some(size_bytes),
                    modified_at,
                    fingerprint,
                    fingerprint_status,
                    error,
                }
            }
            Ok(_) => Self {
                path,
                exists: true,
                readable: false,
                size_bytes: None,
                modified_at: None,
                fingerprint: None,
                fingerprint_status: "error",
                error: Some("path is not a regular file".to_string()),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self {
                path,
                exists: false,
                readable: false,
                size_bytes: None,
                modified_at: None,
                fingerprint: None,
                fingerprint_status: "missing",
                error: None,
            },
            Err(error) => Self {
                path,
                exists: true,
                readable: false,
                size_bytes: None,
                modified_at: None,
                fingerprint: None,
                fingerprint_status: "error",
                error: Some(error.to_string()),
            },
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "path": self.path.display().to_string(),
            "exists": self.exists,
            "readable": self.readable,
            "sizeBytes": self.size_bytes,
            "modifiedAt": self.modified_at,
            "fingerprint": self.fingerprint,
            "fingerprintStatus": self.fingerprint_status,
            "error": self.error
        })
    }
}

fn fingerprint_file(
    path: &Path,
    size_bytes: u64,
) -> (Option<String>, &'static str, Option<String>) {
    if size_bytes > MAX_BINARY_FINGERPRINT_BYTES {
        return (None, "skipped_too_large", None);
    }
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => return (None, "error", Some(error.to_string())),
    };
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    (Some(format!("fnv64:{hash:016x}")), "ok", None)
}

fn doctor_binary(cwd: Option<&str>) -> Value {
    let running = match std::env::current_exe() {
        Ok(path) => BinaryTarget::inspect(path),
        Err(error) => BinaryTarget {
            path: PathBuf::new(),
            exists: false,
            readable: false,
            size_bytes: None,
            modified_at: None,
            fingerprint: None,
            fingerprint_status: "error",
            error: Some(error.to_string()),
        },
    };
    let installed = BinaryTarget::inspect(installed_binary_path());
    let release = BinaryTarget::inspect(release_binary_path(cwd));
    binary_report(running, installed, release)
}

fn installed_binary_path() -> PathBuf {
    env::var("AGENT_BRIDGE_INSTALLED_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|_| expand_home("~/.local/bin/agent-bridge-mcp"))
}

fn release_binary_path(cwd: Option<&str>) -> PathBuf {
    if let Ok(path) = env::var("AGENT_BRIDGE_RELEASE_BIN") {
        return PathBuf::from(path);
    }
    cwd.map(PathBuf::from)
        .or_else(|| env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
        .join("target/release/agent-bridge-mcp")
}

fn binary_report(running: BinaryTarget, installed: BinaryTarget, release: BinaryTarget) -> Value {
    let matches_release = binary_targets_match(&installed, &release);
    let installed_matches_running = binary_targets_match(&installed, &running);
    let release_matches_running = binary_targets_match(&release, &running);
    let status = binary_status(
        &running,
        &installed,
        &release,
        matches_release,
        installed_matches_running,
    );
    let recommendations =
        binary_recommendation_strings(status, matches_release, &installed, &release);
    let mut installed_json = installed.to_json();
    installed_json["matchesRelease"] = json!(matches_release);
    installed_json["matchesRunning"] = json!(installed_matches_running);
    let mut release_json = release.to_json();
    release_json["matchesRunning"] = json!(release_matches_running);
    json!({
        "status": status,
        "fingerprintLimitBytes": MAX_BINARY_FINGERPRINT_BYTES,
        "running": running.to_json(),
        "installed": installed_json,
        "release": release_json,
        "recommendations": recommendations
    })
}

fn binary_targets_match(left: &BinaryTarget, right: &BinaryTarget) -> bool {
    left.readable
        && right.readable
        && left.size_bytes == right.size_bytes
        && left.fingerprint_status == "ok"
        && right.fingerprint_status == "ok"
        && left.fingerprint == right.fingerprint
}

fn binary_status(
    running: &BinaryTarget,
    installed: &BinaryTarget,
    release: &BinaryTarget,
    matches_release: bool,
    installed_matches_running: bool,
) -> &'static str {
    if running.fingerprint_status == "error"
        || override_path_error("AGENT_BRIDGE_INSTALLED_BIN", installed)
        || override_path_error("AGENT_BRIDGE_RELEASE_BIN", release)
    {
        return "error";
    }
    if !installed.exists
        || !release.exists
        || !matches_release
        || !installed_matches_running
        || [running, installed, release]
            .iter()
            .any(|target| target.fingerprint_status == "skipped_too_large")
    {
        return "warning";
    }
    if installed.readable && release.readable && matches_release {
        "ok"
    } else {
        "unknown"
    }
}

fn override_path_error(key: &str, target: &BinaryTarget) -> bool {
    env::var_os(key).is_some()
        && target.exists
        && (!target.readable || target.fingerprint_status == "error")
}

fn binary_recommendation_strings(
    status: &str,
    matches_release: bool,
    installed: &BinaryTarget,
    release: &BinaryTarget,
) -> Vec<String> {
    let mut recommendations = Vec::new();
    if !release.exists {
        recommendations.push(
            "Build the release binary with cargo build --release --bin agent-bridge-mcp before comparing freshness."
                .to_string(),
        );
    }
    if !installed.exists {
        recommendations.push(
            "Install the release binary to the configured installed binary path.".to_string(),
        );
    }
    if status == "warning" && installed.exists && release.exists && !matches_release {
        recommendations.push(
            "Rebuild and install the release binary so the installed copy matches target/release."
                .to_string(),
        );
    }
    recommendations
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClientKind {
    Codex,
    Claude,
    Cursor,
}

impl ClientKind {
    fn name(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Cursor => "cursor",
        }
    }

    fn config_path(self, home: &Path) -> PathBuf {
        match self {
            Self::Codex => home.join(".codex/config.toml"),
            Self::Claude => home.join(".claude.json"),
            Self::Cursor => home.join(".cursor/mcp.json"),
        }
    }

    fn verification_command(self) -> Option<Vec<&'static str>> {
        match self {
            Self::Codex => Some(vec!["codex", "mcp", "list"]),
            Self::Claude => Some(vec!["claude", "mcp", "list"]),
            Self::Cursor => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct ClientRegistration {
    command: Option<String>,
    args: Vec<String>,
    env_keys: Vec<String>,
    similar_registrations: Vec<String>,
}

struct ClientReport {
    client: ClientKind,
    config_path: String,
    config_present: bool,
    parse_status: &'static str,
    registration_status: &'static str,
    command: Option<Value>,
    args: Vec<String>,
    env_keys: Vec<String>,
    similar_registrations: Vec<String>,
    status: &'static str,
    recommendations: Vec<String>,
    error: Option<String>,
}

fn doctor_clients() -> Value {
    let home = env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("~"));
    doctor_clients_from_home(&home)
}

fn doctor_clients_from_home(home: &Path) -> Value {
    let mut clients = serde_json::Map::new();
    for client in [ClientKind::Codex, ClientKind::Claude, ClientKind::Cursor] {
        clients.insert(client.name().to_string(), doctor_client(client, home));
    }
    Value::Object(clients)
}

fn doctor_client(client: ClientKind, home: &Path) -> Value {
    let path = client.config_path(home);
    let path_text = path.display().to_string();
    let contents = match read_client_config(&path) {
        Ok(Some(contents)) => contents,
        Ok(None) => {
            return client_report(ClientReport {
                client,
                config_path: path_text.clone(),
                config_present: false,
                parse_status: "missing",
                registration_status: "absent",
                command: None,
                args: Vec::new(),
                env_keys: Vec::new(),
                similar_registrations: Vec::new(),
                status: "info",
                recommendations: vec![format!(
                    "No {} user-level MCP config file was found; add Agent Bridge there only if you use this client.",
                    client.name()
                )],
                error: None,
            });
        }
        Err(error) => {
            return client_report(ClientReport {
                client,
                config_path: path_text.clone(),
                config_present: true,
                parse_status: "error",
                registration_status: "absent",
                command: None,
                args: Vec::new(),
                env_keys: Vec::new(),
                similar_registrations: Vec::new(),
                status: "error",
                recommendations: vec![format!(
                    "Inspect {} because it could not be read: {error}.",
                    path_text,
                )],
                error: Some(error),
            });
        }
    };

    let parsed = match client {
        ClientKind::Codex => parse_codex_registration(&contents),
        ClientKind::Claude | ClientKind::Cursor => parse_json_registration(&contents),
    };
    let registration = match parsed {
        Ok(Some(registration)) => registration,
        Ok(None) => {
            return client_report(ClientReport {
                client,
                config_path: path_text.clone(),
                config_present: true,
                parse_status: "ok",
                registration_status: "absent",
                command: None,
                args: Vec::new(),
                env_keys: Vec::new(),
                similar_registrations: Vec::new(),
                status: "info",
                recommendations: vec![format!(
                    "No canonical agent-bridge MCP registration was found in {}.",
                    path_text,
                )],
                error: None,
            });
        }
        Err(error) => {
            return client_report(ClientReport {
                client,
                config_path: path_text,
                config_present: true,
                parse_status: "error",
                registration_status: "absent",
                command: None,
                args: Vec::new(),
                env_keys: Vec::new(),
                similar_registrations: Vec::new(),
                status: "error",
                recommendations: vec![format!("Fix the {} config parse error.", client.name())],
                error: Some(error),
            });
        }
    };

    let command = command_diagnostic(registration.command.as_deref());
    let status = if command["status"].as_str() == Some("warning") {
        "warning"
    } else {
        "ok"
    };
    let mut recommendations = Vec::new();
    if status == "warning" {
        recommendations.push(format!(
            "Inspect the {} Agent Bridge command configuration.",
            client.name()
        ));
    }
    let verification_commands = verification_commands(client);
    if !verification_commands.is_empty() {
        recommendations.push(format!(
            "Run {} to verify the {} client can load the registered MCP server.",
            verification_commands[0]["command"]
                .as_array()
                .map(|parts| {
                    parts
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default(),
            client.name()
        ));
    }
    client_report(ClientReport {
        client,
        config_path: path_text,
        config_present: true,
        parse_status: "ok",
        registration_status: "registered",
        command: Some(command),
        args: registration.args,
        env_keys: registration.env_keys,
        similar_registrations: registration.similar_registrations,
        status,
        recommendations,
        error: None,
    })
}

fn read_client_config(path: &Path) -> Result<Option<String>, String> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    if metadata.len() > MAX_CLIENT_CONFIG_BYTES {
        return Err(format!(
            "config file exceeds {} bytes",
            MAX_CLIENT_CONFIG_BYTES
        ));
    }
    std::fs::read_to_string(path)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn client_report(mut report: ClientReport) -> Value {
    report.env_keys.sort();
    report.env_keys.dedup();
    let verification_commands = if report.registration_status == "registered" {
        verification_commands(report.client)
    } else {
        Vec::new()
    };
    let mut value = json!({
        "client": report.client.name(),
        "status": report.status,
        "configPath": report.config_path,
        "configPresent": report.config_present,
        "parseStatus": report.parse_status,
        "registrationStatus": report.registration_status,
        "command": report.command.unwrap_or_else(|| command_diagnostic(None)),
        "args": report.args,
        "envKeys": report.env_keys,
        "verificationStatus": "not_verified",
        "verificationCommands": verification_commands,
        "recommendations": report.recommendations
    });
    if !report.similar_registrations.is_empty() {
        value["similarRegistrations"] = json!(report.similar_registrations);
    }
    if let Some(error) = report.error {
        value["error"] = json!(error);
    }
    value
}

fn verification_commands(client: ClientKind) -> Vec<Value> {
    client
        .verification_command()
        .into_iter()
        .map(|command| {
            json!({
                "kind": "shell",
                "command": command,
                "description": format!(
                    "Verify {} can load the registered Agent Bridge MCP server.",
                    client.name()
                )
            })
        })
        .collect()
}

fn command_diagnostic(command: Option<&str>) -> Value {
    let Some(command) = command.filter(|command| !command.is_empty()) else {
        return json!({
            "value": null,
            "status": "warning",
            "resolution": "missing",
            "message": "Agent Bridge registration does not define a command string."
        });
    };
    let path = Path::new(command);
    if path.is_absolute() {
        if path.exists() {
            json!({
                "value": command,
                "status": "ok",
                "resolution": "absolute_exists"
            })
        } else {
            json!({
                "value": command,
                "status": "warning",
                "resolution": "absolute_missing",
                "message": "Configured absolute command path does not exist."
            })
        }
    } else {
        json!({
            "value": command,
            "status": "info",
            "resolution": "path_lookup_required",
            "message": "Command is not absolute; the client PATH controls resolution."
        })
    }
}

fn parse_json_registration(contents: &str) -> Result<Option<ClientRegistration>, String> {
    let value: Value = serde_json::from_str(contents).map_err(|error| error.to_string())?;
    let Some(servers) = value.get("mcpServers").and_then(Value::as_object) else {
        return Ok(None);
    };
    let similar_registrations = similar_json_registrations(servers);
    let Some(entry) = servers.get("agent-bridge").and_then(Value::as_object) else {
        return Ok(None);
    };
    Ok(Some(ClientRegistration {
        command: entry
            .get("command")
            .and_then(Value::as_str)
            .map(str::to_string),
        args: entry
            .get("args")
            .and_then(Value::as_array)
            .map(|values| string_array(values))
            .unwrap_or_default(),
        env_keys: entry
            .get("env")
            .and_then(Value::as_object)
            .map(|env| env.keys().cloned().collect())
            .unwrap_or_default(),
        similar_registrations,
    }))
}

fn similar_json_registrations(servers: &serde_json::Map<String, Value>) -> Vec<String> {
    servers
        .iter()
        .filter(|(name, _)| name.as_str() != "agent-bridge")
        .filter_map(|(name, value)| {
            value
                .get("command")
                .and_then(Value::as_str)
                .filter(|command| command.contains("agent-bridge-mcp"))
                .map(|_| name.clone())
        })
        .collect()
}

fn string_array(values: &[Value]) -> Vec<String> {
    values
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn parse_codex_registration(contents: &str) -> Result<Option<ClientRegistration>, String> {
    let mut current_section = String::new();
    let mut registration = ClientRegistration::default();
    let mut found = false;
    let mut similar = Vec::new();
    for line in contents.lines() {
        let line = strip_toml_comment(line).trim().to_string();
        if line.is_empty() {
            continue;
        }
        if let Some(section) = toml_section(&line) {
            current_section = section;
            continue;
        }
        let Some((key, value)) = toml_assignment(&line) else {
            continue;
        };
        if is_codex_agent_bridge_section(&current_section) {
            found = true;
            match key.as_str() {
                "command" => registration.command = parse_toml_string(&value),
                "args" => registration.args = parse_toml_string_array(&value),
                "env" => registration
                    .env_keys
                    .extend(parse_toml_inline_table_keys(&value)),
                _ => {}
            }
        } else if is_codex_agent_bridge_env_section(&current_section) {
            found = true;
            registration.env_keys.push(key);
        } else if is_codex_mcp_server_section(&current_section)
            && key == "command"
            && parse_toml_string(&value).is_some_and(|command| command.contains("agent-bridge-mcp"))
            && let Some(name) = codex_mcp_server_name(&current_section)
            && name != "agent-bridge"
        {
            similar.push(name);
        }
    }
    registration.similar_registrations = similar;
    if found {
        Ok(Some(registration))
    } else {
        Ok(None)
    }
}

fn strip_toml_comment(line: &str) -> String {
    let mut in_quote = false;
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' if in_quote => escaped = true,
            '"' => in_quote = !in_quote,
            '#' if !in_quote => return line[..index].to_string(),
            _ => {}
        }
    }
    line.to_string()
}

fn toml_section(line: &str) -> Option<String> {
    line.strip_prefix('[')
        .and_then(|line| line.strip_suffix(']'))
        .map(|section| section.trim().to_string())
}

fn toml_assignment(line: &str) -> Option<(String, String)> {
    let (key, value) = line.split_once('=')?;
    Some((unquote_toml_key(key.trim()), value.trim().to_string()))
}

fn unquote_toml_key(key: &str) -> String {
    key.trim_matches('"').trim_matches('\'').to_string()
}

fn is_codex_agent_bridge_section(section: &str) -> bool {
    matches!(
        section,
        "mcp_servers.agent-bridge" | "mcp_servers.\"agent-bridge\"" | "mcp_servers.'agent-bridge'"
    )
}

fn is_codex_agent_bridge_env_section(section: &str) -> bool {
    matches!(
        section,
        "mcp_servers.agent-bridge.env"
            | "mcp_servers.\"agent-bridge\".env"
            | "mcp_servers.'agent-bridge'.env"
    )
}

fn is_codex_mcp_server_section(section: &str) -> bool {
    section.starts_with("mcp_servers.") && !section.ends_with(".env")
}

fn codex_mcp_server_name(section: &str) -> Option<String> {
    section
        .strip_prefix("mcp_servers.")
        .map(|name| unquote_toml_key(name.trim()))
}

fn parse_toml_string(value: &str) -> Option<String> {
    let value = value.trim();
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .map(|value| value.replace("\\\"", "\"").replace("\\\\", "\\"))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
                .map(str::to_string)
        })
}

fn parse_toml_string_array(value: &str) -> Vec<String> {
    let Some(inner) = value
        .trim()
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
    else {
        return Vec::new();
    };
    inner
        .split(',')
        .filter_map(|item| parse_toml_string(item.trim()))
        .collect()
}

fn parse_toml_inline_table_keys(value: &str) -> Vec<String> {
    let Some(inner) = value
        .trim()
        .strip_prefix('{')
        .and_then(|v| v.strip_suffix('}'))
    else {
        return Vec::new();
    };
    inner
        .split(',')
        .filter_map(|item| {
            item.split_once('=')
                .map(|(key, _)| unquote_toml_key(key.trim()))
        })
        .collect()
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
            "launchStrategy": "host_runner_required"
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
    launch_readiness: &Value,
    clients: &Value,
    binary: &Value,
) -> Value {
    let mut recommendations = Vec::new();
    if workspace_status == "error" {
        recommendations.push(json!({
            "id": "configure_workspace",
            "severity": "error",
            "message": "Set AGENT_BRIDGE_WORKSPACES or pass a cwd inside a configured workspace.",
            "tool": "doctor",
            "arguments": {}
        }));
    }
    if state_status == "error" {
        recommendations.push(json!({
            "id": "fix_state_dir",
            "severity": "error",
            "message": "Fix AGENT_BRIDGE_STATE_DIR so Agent Bridge can read and write task state.",
            "tool": "doctor",
            "arguments": {}
        }));
    } else if state_status == "warning" {
        recommendations.push(json!({
            "id": "create_state_dir",
            "severity": "warning",
            "message": "Create AGENT_BRIDGE_STATE_DIR before spawning delegated tasks.",
            "tool": "doctor",
            "arguments": {}
        }));
    }
    if host_status == "error" {
        recommendations.push(json!({
            "id": "restart_claude_host_runner",
            "severity": "warning",
            "message": "Restart or reconfigure the Claude host runner with matching AGENT_BRIDGE_WORKSPACES, then rerun doctor.",
            "tool": "doctor",
            "arguments": {}
        }));
    }
    if provider_status == "warning" {
        recommendations.push(json!({
            "id": "fix_unavailable_providers",
            "severity": "warning",
            "message": "Install or configure unavailable providers, or pass providers to focus doctor output.",
            "tool": "providers_check",
            "arguments": {}
        }));
    }
    let stale_providers: Vec<_> = launch_readiness["providers"]
        .as_object()
        .map(|providers| {
            providers
                .iter()
                .filter(|(_, provider)| {
                    provider["available"].as_bool() == Some(true)
                        && provider["startupVerified"].as_bool() == Some(false)
                })
                .map(|(name, _)| name.clone())
                .collect()
        })
        .unwrap_or_default();
    if !stale_providers.is_empty() {
        recommendations.push(json!({
            "id": "verify_provider_startup",
            "severity": "info",
            "message": "Selected providers are version-available but not startup-verified; run a bounded smoke check before first launch when startup readiness matters.",
            "tool": "providers_check",
            "arguments": {
                "providers": stale_providers,
                "smoke": true
            }
        }));
    }
    recommendations.extend(client_recommendations(clients));
    recommendations.extend(binary_recommendations(binary));
    Value::Array(recommendations)
}

fn binary_recommendations(binary: &Value) -> Vec<Value> {
    let mut recommendations = Vec::new();
    let status = binary["status"].as_str().unwrap_or("unknown");
    if status == "ok" {
        return recommendations;
    }
    if binary["release"]["exists"].as_bool() == Some(false) {
        recommendations.push(json!({
            "id": "build_release_binary",
            "severity": "info",
            "kind": "shell",
            "command": ["cargo", "build", "--release", "--bin", "agent-bridge-mcp"],
            "message": "Build the release Agent Bridge binary before comparing or installing binary freshness."
        }));
    }
    if binary["installed"]["exists"].as_bool() == Some(false)
        || binary["installed"]["matchesRelease"].as_bool() == Some(false)
    {
        let installed_path = binary["installed"]["path"]
            .as_str()
            .unwrap_or("~/.local/bin/agent-bridge-mcp");
        recommendations.push(json!({
            "id": "install_release_binary",
            "severity": "info",
            "kind": "shell",
            "command": ["install", "-m", "0755", "target/release/agent-bridge-mcp", installed_path],
            "message": "Install the release Agent Bridge binary after building it."
        }));
    }
    recommendations
}

fn client_recommendations(clients: &Value) -> Vec<Value> {
    let Some(clients) = clients.as_object() else {
        return Vec::new();
    };
    let mut recommendations = Vec::new();
    for (name, client) in clients {
        match client["registrationStatus"].as_str() {
            Some("registered") => {
                if let Some(command) = client["verificationCommands"]
                    .as_array()
                    .and_then(|commands| commands.first())
                    .and_then(|command| command["command"].as_array())
                {
                    recommendations.push(json!({
                        "id": format!("verify_{name}_client_config"),
                        "severity": "info",
                        "kind": "shell",
                        "command": command,
                        "message": format!("Run {} to verify the {name} client can load Agent Bridge.", command.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" "))
                    }));
                }
            }
            Some("absent") if matches!(client["parseStatus"].as_str(), Some("ok" | "missing")) => {
                recommendations.push(json!({
                    "id": format!("configure_{name}_client"),
                    "severity": "info",
                    "message": format!("Add Agent Bridge to the {name} user-level MCP config only if you use that client.")
                }));
            }
            _ if client["parseStatus"].as_str() == Some("error") => {
                recommendations.push(json!({
                    "id": format!("fix_{name}_client_config"),
                    "severity": "warning",
                    "message": format!("Fix the {name} MCP config parse/read error before relying on that client.")
                }));
            }
            _ => {}
        }
    }
    recommendations
}

fn doctor_launch_readiness(providers: &Value, selected: Option<&[ProviderKind]>) -> Value {
    let mut provider_readiness = serde_json::Map::new();
    let mut any_not_verified = false;
    let mut all_launchable = true;
    if let Some(providers) = providers.as_object() {
        for (name, provider) in providers {
            let available = provider["available"].as_bool().unwrap_or(false);
            let startup_verified = provider["startupVerified"].as_bool().unwrap_or(false);
            let launchable = provider["launchable"].as_bool().unwrap_or(false);
            any_not_verified |= available && !startup_verified;
            all_launchable &= launchable;
            provider_readiness.insert(
                name.clone(),
                json!({
                    "available": available,
                    "startupVerified": startup_verified,
                    "launchable": launchable,
                    "readiness": provider.get("readiness").cloned().unwrap_or_else(|| json!({}))
                }),
            );
        }
    }
    let selected_providers = selected
        .map(|providers| {
            providers
                .iter()
                .map(|provider| provider.as_str())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "status": if all_launchable {
            "ready"
        } else if any_not_verified {
            "not_verified"
        } else {
            "not_launchable"
        },
        "startupVerified": !any_not_verified && all_launchable,
        "launchable": all_launchable,
        "selectedProviders": selected_providers,
        "providers": provider_readiness
    })
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
        let checked_at = checked_at_iso();
        let value = match output.status {
            Some(status) if status.success() && output.failure_category.is_none() => {
                let mut value = json!({
                    "available": true,
                    "command": command.command,
                    "version": String::from_utf8_lossy(&output.stdout).trim(),
                    "probe": if input.smoke { "version+smoke" } else { "version" },
                    "startupVerified": false,
                    "launchable": false,
                    "checkedAt": checked_at,
                    "versionDurationMs": output.duration_ms
                });
                set_readiness(&mut value, "stale", "version", false, false, None);
                value
            }
            _ => {
                let diagnostic = provider_diagnostic(
                    provider,
                    &command,
                    &output,
                    VERSION_TIMEOUT_MS,
                    false,
                    "version",
                );
                let mut value = json!({
                    "available": false,
                    "command": command.command,
                    "probe": "version",
                    "startupVerified": false,
                    "launchable": false,
                    "checkedAt": checked_at,
                    "error": probe_error_text(&output),
                    "versionDurationMs": output.duration_ms,
                    "diagnostic": diagnostic
                });
                let diagnostic = value["diagnostic"].clone();
                set_readiness(
                    &mut value,
                    "failed",
                    "version",
                    false,
                    false,
                    Some(diagnostic),
                );
                value
            }
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
        ProviderKind::Claude => 60_000,
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
        value["launchable"] = json!(false);
        value["checkedAt"] = json!(checked_at_iso());
        value["error"] =
            json!("aggregate provider readiness timeout expired before smoke probe started");
        value["diagnostic"] = json!({
            "failureCategory": "provider_timeout",
            "provider": provider.as_str(),
            "startupVerified": false,
            "timeoutMs": aggregate_timeout_ms,
            "phase": "smoke"
        });
        let diagnostic = value["diagnostic"].clone();
        set_readiness(
            &mut value,
            "failed",
            "smoke",
            false,
            false,
            Some(diagnostic),
        );
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
                    value["launchable"] = json!(true);
                    value["checkedAt"] = json!(checked_at_iso());
                    value["smokeDurationMs"] = json!(output.duration_ms);
                    value["smokePromptStrategy"] = json!(strategy);
                    if provider == ProviderKind::Claude {
                        value["launchStrategy"] = json!(launch_strategy(provider));
                    }
                    set_readiness(&mut value, "ready", "version+smoke", true, true, None);
                    value
                }
                _ => {
                    let mut value = base_value;
                    value["available"] = json!(false);
                    value["startupVerified"] = json!(false);
                    value["launchable"] = json!(false);
                    value["checkedAt"] = json!(checked_at_iso());
                    value["smokeDurationMs"] = json!(output.duration_ms);
                    value["smokePromptStrategy"] = json!(strategy);
                    if provider == ProviderKind::Claude {
                        value["launchStrategy"] = json!(launch_strategy(provider));
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
                    let diagnostic = value["diagnostic"].clone();
                    set_readiness(
                        &mut value,
                        "failed",
                        "version+smoke",
                        false,
                        false,
                        Some(diagnostic),
                    );
                    value
                }
            }
        }
        Err(error) => {
            let mut value = base_value;
            value["available"] = json!(false);
            value["startupVerified"] = json!(false);
            value["launchable"] = json!(false);
            value["checkedAt"] = json!(checked_at_iso());
            value["error"] = json!(error);
            set_readiness(&mut value, "failed", "version+smoke", false, false, None);
            value
        }
    };
    (provider, smoke_value)
}

fn set_readiness(
    value: &mut Value,
    state: &'static str,
    probe: &'static str,
    startup_verified: bool,
    launchable: bool,
    diagnostic: Option<Value>,
) {
    let mut readiness = json!({
        "state": state,
        "startupVerified": startup_verified,
        "launchable": launchable,
        "probe": probe,
        "checkedAt": value["checkedAt"],
        "versionDurationMs": value["versionDurationMs"]
    });
    if value.get("smokeDurationMs").is_some() {
        readiness["smokeDurationMs"] = value["smokeDurationMs"].clone();
    }
    if let Some(diagnostic) = diagnostic {
        readiness["diagnostic"] = diagnostic;
    }
    value["readiness"] = readiness;
}

fn checked_at_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
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
            "Start the Agent Bridge Claude host runner and retry providers_check with smoke: true"
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
