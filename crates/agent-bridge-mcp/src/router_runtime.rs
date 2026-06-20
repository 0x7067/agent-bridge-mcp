use crate::domain::{FailureCategory, ProviderKind, TaskMode};
use crate::mcp::{JsonRpcId, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use crate::router::{
    AttemptDisposition, AttemptEvidence, RoutedAttemptInput, RouterPolicy, RouterStopReason,
    classify_attempt,
};
use crate::task::TaskManagerHandle;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use uuid::Uuid;

const ROUTER_EVIDENCE_UPDATE_LIMIT: usize = 20;

pub async fn run_acp_router() -> io::Result<()> {
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
    let turn = RouterPromptTurn {
        session_id: params.session_id,
        cwd: session.cwd.clone(),
        prompt,
        policy,
        mode: params.mode.unwrap_or(TaskMode::Implement),
        timeout_seconds: params.timeout_seconds,
    };
    match execute_router_turn(turn, RouterUpdateSink::Acp { stdout }).await {
        Ok(result) => Ok(Some(JsonRpcResponse::result(
            id,
            json!({
                "stopReason": result.stop_reason,
                "routerResult": result.router_result
            }),
        ))),
        Err(error) => Ok(Some(error.into_json_rpc_response(id))),
    }
}

pub(crate) struct RouterPromptTurn {
    pub session_id: String,
    pub cwd: Option<String>,
    pub prompt: String,
    pub policy: RouterPolicy,
    pub mode: TaskMode,
    pub timeout_seconds: Option<i64>,
}

#[allow(dead_code)]
pub(crate) enum RouterUpdateSink<'a> {
    Acp { stdout: &'a mut io::Stdout },
    Silent,
}

pub(crate) struct RouterTerminalResult {
    pub stop_reason: &'static str,
    pub router_result: Value,
}

#[allow(dead_code)]
pub(crate) enum RouterPromptError {
    InvalidParams(String),
    Runtime(String),
}

impl From<std::io::Error> for RouterPromptError {
    fn from(error: std::io::Error) -> Self {
        Self::Runtime(error.to_string())
    }
}

impl RouterPromptError {
    pub(crate) fn into_json_rpc_response(self, id: JsonRpcId) -> JsonRpcResponse {
        match self {
            Self::InvalidParams(message) => JsonRpcResponse::error(id, -32602, message),
            Self::Runtime(message) => JsonRpcResponse::error(id, -32000, message),
        }
    }
}

pub(crate) async fn execute_router_turn(
    turn: RouterPromptTurn,
    mut updates: RouterUpdateSink<'_>,
) -> Result<RouterTerminalResult, RouterPromptError> {
    let manager = TaskManagerHandle::from_env()
        .await
        .map_err(RouterPromptError::Runtime)?;
    let timeout_ms = turn
        .timeout_seconds
        .map(|seconds| seconds.saturating_mul(1_000).saturating_add(1_000));
    let mut attempts = Vec::new();
    let mut evidence_refs = Vec::new();
    let mut failover_trail = Vec::new();
    let mut pending_failover: Option<Value> = None;
    let candidate_count = turn.policy.candidates.len();
    for (index, provider) in turn.policy.candidates.iter().copied().enumerate() {
        let execution = manager
            .run_router_attempt(
                RoutedAttemptInput {
                    provider,
                    mode: turn.mode,
                    prompt: turn.prompt.clone(),
                    title: Some("ACP router turn".to_string()),
                    cwd: turn.cwd.clone(),
                    timeout_seconds: turn.timeout_seconds,
                    isolation: None,
                    worktree_name: None,
                    profile: None,
                },
                timeout_ms,
            )
            .await
            .map_err(RouterPromptError::Runtime)?;
        let transcript = manager
            .transcript(execution.agent_id.clone(), None, Some(500))
            .await
            .ok();
        if let Some(transcript) = transcript.as_ref()
            && let RouterUpdateSink::Acp { stdout } = &mut updates
        {
            write_router_evidence_updates(stdout, &turn.session_id, provider, transcript).await?;
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
        if let Some(text) = routed_final_text.as_deref()
            && let RouterUpdateSink::Acp { stdout } = &mut updates
        {
            let notification = JsonRpcNotification::new(
                "session/update",
                json!({
                    "sessionId": turn.session_id,
                    "update": {
                        "sessionUpdate": "agent_message_chunk",
                        "content": {"type": "text", "text": text}
                    }
                }),
            );
            write_json_message(stdout, &notification).await?;
        }
        let terminal_kind = router_terminal_kind(disposition);
        return Ok(RouterTerminalResult {
            stop_reason: response_stop_reason,
            router_result: json!({
                "provider": provider,
                "terminalKind": terminal_kind,
                "finalText": routed_final_text,
                "failureCategory": failure_category,
                "blockerReason": router_blocker_reason(disposition, stop_reason, failure_category),
                "verificationStatus": "not_verified",
                "attempts": attempts.clone(),
                "evidenceRefs": evidence_refs.clone(),
                "diagnostics": {
                    "provider": provider,
                    "terminalKind": terminal_kind,
                    "attempts": attempts,
                    "failoverTrail": failover_trail,
                    "evidenceRefs": evidence_refs,
                    "bounded": true
                }
            }),
        });
    }
    Err(RouterPromptError::Runtime(
        "router policy did not produce an attempt".to_string(),
    ))
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

async fn write_response(stdout: &mut io::Stdout, response: &JsonRpcResponse) -> io::Result<()> {
    write_json_message(stdout, response).await
}

async fn write_json_message<T: serde::Serialize>(
    stdout: &mut io::Stdout,
    message: &T,
) -> io::Result<()> {
    let mut line = serde_json::to_vec(message).map_err(io::Error::other)?;
    line.push(b'\n');
    stdout.write_all(&line).await?;
    stdout.flush().await
}
