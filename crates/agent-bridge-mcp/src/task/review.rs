use super::complete::{provider_env_redactions, redact_value};
use super::{
    MAX_LOG_BYTES, MAX_OBSERVE_EVENTS, MAX_OBSERVE_MS, MAX_WAIT_MS, PROGRESS_TRANSCRIPT_TAIL_BYTES,
    Registry, ResultSections, TaskListInput, TaskListScope, TaskRecord,
};
use crate::domain::{ErrorType, PartialResult, TaskPhase, TaskStatus};
use crate::provider::{self};
use chrono::Utc;
use serde_json::{Value, json};
use std::cmp::Ordering;
use std::io::{BufRead, BufReader as StdBufReader, ErrorKind, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

pub(crate) const AGENT_COMPLETED_NOTIFICATION: &str = "notifications/agent_bridge/agent_completed";

pub(super) fn parse_transcript_line(line: &str) -> (&'static str, Value) {
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

pub(super) async fn read_transcript(
    task: &TaskRecord,
    cursor: usize,
    limit: usize,
) -> Result<Value, String> {
    let path = PathBuf::from(&task.agent_dir).join("transcript.jsonl");
    let max_events = limit.clamp(1, 500);
    let file = match fs::File::open(&path).await {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(json!({
                "agentId": task.agent_id,
                "available": false,
                "events": [],
                "nextCursor": cursor,
                "message": "transcript not available"
            }));
        }
        Err(error) => return Err(error.to_string()),
    };
    let mut reader = BufReader::new(file);
    let mut events = Vec::new();
    let mut line = Vec::new();
    let mut line_index = 0usize;
    let mut truncated = false;
    loop {
        line.clear();
        let bytes = reader
            .read_until(b'\n', &mut line)
            .await
            .map_err(|error| error.to_string())?;
        if bytes == 0 {
            break;
        }
        if line_index >= cursor {
            if events.len() >= max_events {
                truncated = true;
                break;
            }
            trim_jsonl_line(&mut line);
            let mut event =
                parse_jsonl_value(&line).unwrap_or_else(|| json!({"kind": "malformed"}));
            mark_result_event(&mut event, &task.partial_results);
            event = redact_value(event, &provider_env_redactions(task.provider));
            event["index"] = json!(line_index);
            events.push(event);
        }
        line_index += 1;
    }
    let next_cursor = if events.is_empty() {
        cursor.min(line_index)
    } else {
        cursor + events.len()
    };
    Ok(json!({
        "agentId": task.agent_id,
        "available": true,
        "events": events,
        "nextCursor": next_cursor,
        "truncated": truncated
    }))
}

fn mark_result_event(event: &mut Value, partial_results: &[PartialResult]) {
    if is_final_result_event(event) {
        event["finalResult"] = json!(true);
        return;
    }
    if partial_results
        .iter()
        .any(|partial| event_matches_partial_result(event, partial))
    {
        event["partialResult"] = json!(true);
    }
}

fn is_final_result_event(event: &Value) -> bool {
    event.get("kind").and_then(Value::as_str) == Some("provider_result")
        || (event.get("type").and_then(Value::as_str) == Some("result")
            && event.get("result").and_then(Value::as_str).is_some())
}

fn event_matches_partial_result(event: &Value, partial: &PartialResult) -> bool {
    let source = event
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let kind = event
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("provider_event");
    source == partial.source
        && kind == partial.kind
        && event_timestamp(event) == partial.timestamp
        && event_summary(event) == partial.summary
}

fn event_timestamp(event: &Value) -> String {
    event
        .get("ts")
        .or_else(|| event.get("timestamp"))
        .or_else(|| event.get("at"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn event_summary(event: &Value) -> String {
    let summary = event
        .get("raw")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| event.get("parsed").map(|value| value.to_string()))
        .unwrap_or_default();
    summary.chars().take(512).collect()
}

pub(super) fn transcript_evidence(agent_dir: &str) -> (bool, bool, bool) {
    let path = PathBuf::from(agent_dir).join("transcript.jsonl");
    let Ok(file) = std::fs::File::open(path) else {
        return (false, false, false);
    };
    let mut reader = StdBufReader::new(file);
    let mut has_event = false;
    let mut has_provider_output = false;
    let mut has_result = false;
    let mut line = Vec::new();
    for_each_jsonl_value(&mut reader, &mut line, |value| {
        has_event = true;
        let kind = value.get("kind").and_then(Value::as_str);
        let source = value.get("source").and_then(Value::as_str);
        if matches!(source, Some("stdout" | "stderr")) {
            has_provider_output = true;
        }
        if kind == Some("provider_result") {
            has_result = true;
        }
    });
    (has_event, has_result, has_provider_output && !has_result)
}

pub(super) fn list_tasks(registry: &Registry, arguments: Value) -> Result<Value, String> {
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
    let attention_inbox = presentation && scope == TaskListScope::ActiveRecent;
    let hide_inspected_finals = attention_inbox && !has_explicit_list_filters(&input);
    let mut tasks: Vec<&TaskRecord> = registry
        .tasks
        .values()
        .filter(|task| task.status != TaskStatus::Removed)
        .filter(|task| agent_matches_list_filters(task, &input))
        .filter(|task| {
            !(hide_inspected_finals && is_final(task.status) && task.result_inspected_at.is_some())
        })
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

pub(super) fn agent_matches_list_filters(task: &TaskRecord, input: &TaskListInput) -> bool {
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

fn has_explicit_list_filters(input: &TaskListInput) -> bool {
    input.status.is_some()
        || input.provider.is_some()
        || input.mode.is_some()
        || input.cwd.is_some()
        || input.title_contains.is_some()
}

pub(super) fn agent_matches_cwd(task: &TaskRecord, cwd: &str) -> bool {
    if task.cwd == cwd || task.original_cwd.as_deref() == Some(cwd) {
        return true;
    }
    let Ok(canonical) = Path::new(cwd).canonicalize() else {
        return false;
    };
    let canonical = canonical.display().to_string();
    task.cwd == canonical || task.original_cwd.as_deref() == Some(canonical.as_str())
}

pub(super) fn compare_for_presentation_list(left: &&TaskRecord, right: &&TaskRecord) -> Ordering {
    presentation_rank(left)
        .cmp(&presentation_rank(right))
        .then_with(|| right.updated_at.cmp(&left.updated_at))
        .then_with(|| left.agent_id.cmp(&right.agent_id))
}

fn presentation_rank(task: &TaskRecord) -> u8 {
    if !is_final(task.status) {
        0
    } else if task.result_inspected_at.is_none() {
        1
    } else {
        2
    }
}

pub(super) struct TranscriptProgressSnapshot {
    last_event_at: Option<String>,
    last_output_at: Option<String>,
}

pub(super) fn agent_progress(task: &TaskRecord) -> Value {
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

pub(super) fn cadence_i64(cadence: &Value, key: &str, default: i64) -> i64 {
    cadence.get(key).and_then(Value::as_i64).unwrap_or(default)
}

pub(super) fn millis_since(timestamp: &str, now: chrono::DateTime<Utc>) -> Option<i64> {
    let then = chrono::DateTime::parse_from_rfc3339(timestamp)
        .ok()?
        .with_timezone(&Utc);
    Some((now - then).num_milliseconds())
}

pub(super) fn transcript_progress_snapshot(task: &TaskRecord) -> TranscriptProgressSnapshot {
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
    let mut reader = StdBufReader::new(file);
    let mut line = Vec::new();
    if start > 0 {
        line.clear();
        if reader.read_until(b'\n', &mut line).is_err() {
            return TranscriptProgressSnapshot {
                last_event_at: None,
                last_output_at: None,
            };
        }
    }
    let mut snapshot = TranscriptProgressSnapshot {
        last_event_at: None,
        last_output_at: None,
    };
    for_each_jsonl_value(&mut reader, &mut line, |value| {
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
    });
    snapshot
}

fn for_each_jsonl_value(
    reader: &mut impl BufRead,
    line: &mut Vec<u8>,
    mut visit: impl FnMut(Value),
) {
    loop {
        line.clear();
        let Ok(bytes) = reader.read_until(b'\n', line) else {
            break;
        };
        if bytes == 0 {
            break;
        }
        trim_jsonl_line(line);
        if let Some(value) = parse_jsonl_value(line) {
            visit(value);
        }
    }
}

fn trim_jsonl_line(line: &mut Vec<u8>) {
    while matches!(line.last(), Some(b'\n' | b'\r')) {
        line.pop();
    }
}

fn parse_jsonl_value(line: &[u8]) -> Option<Value> {
    if line.iter().all(u8::is_ascii_whitespace) {
        return None;
    }
    std::str::from_utf8(line)
        .ok()
        .and_then(|line| serde_json::from_str::<Value>(line).ok())
}

pub(super) fn observe_payload(
    task: TaskRecord,
    transcript: Value,
    timed_out: bool,
    detailed: bool,
) -> Value {
    let progress = agent_progress(&task);
    let events = transcript["events"].as_array().cloned().unwrap_or_default();
    let next = next_actions(&task, &progress);
    let mut value = json!({
        "agentId": task.agent_id,
        "status": task.status,
        "isFinal": is_final(task.status),
        "phase": task_phase(task.status),
        "progress": progress.clone(),
        "timeline": agent_timeline(&task, &events, &progress),
        "events": transcript["events"],
        "nextCursor": transcript["nextCursor"],
        "timedOut": timed_out,
        "next": next
    });
    if detailed {
        add_detail(&mut value, &task);
    }
    value
}

/// Lean agent-facing state envelope. Each field appears once; GUI presentation
/// chrome is intentionally omitted (only LLM callers consume this surface).
pub(super) fn public_task(task: &TaskRecord) -> Value {
    let progress = agent_progress(task);
    let next = next_actions(task, &progress);
    json!({
        "agentId": task.agent_id,
        "status": task.status,
        "isFinal": is_final(task.status),
        "phase": task_phase(task.status),
        "progress": progress.clone(),
        "timeline": agent_timeline(task, &[], &progress),
        "next": next
    })
}

pub(super) fn agent_timeline(task: &TaskRecord, events: &[Value], progress: &Value) -> Value {
    let next = next_actions(task, progress);
    let state = timeline_state(task, events, progress);
    let attention = timeline_attention(task, state);
    let highlights = timeline_highlights(events);
    json!({
        "headline": timeline_headline(task, state),
        "state": state,
        "currentActivity": highlights.first().cloned().map(Value::String).unwrap_or(Value::Null),
        "recentHighlights": highlights.into_iter().map(Value::String).collect::<Vec<_>>(),
        "attention": attention,
        "next": next
    })
}

fn timeline_state(task: &TaskRecord, events: &[Value], progress: &Value) -> &'static str {
    if is_final(task.status) {
        return "final";
    }
    if task.status == TaskStatus::Queued {
        return "queued";
    }
    if progress.get("stallRisk").and_then(Value::as_str) == Some("high") {
        return "stalled";
    }
    if !events.is_empty() {
        return "working";
    }
    "quiet"
}

fn timeline_attention(task: &TaskRecord, state: &str) -> &'static str {
    if is_final(task.status) {
        "read_result"
    } else if state == "stalled" {
        "inspect"
    } else {
        "wait"
    }
}

fn timeline_headline(task: &TaskRecord, state: &str) -> String {
    let title = display_title(task);
    match state {
        "queued" => format!("{title} is queued."),
        "working" => format!("{title} is working."),
        "quiet" => format!("{title} is running quietly."),
        "stalled" => format!("{title} needs attention."),
        "final" => format!("{title} reached a final state."),
        _ => format!("{title} status is available."),
    }
}

fn timeline_highlights(events: &[Value]) -> Vec<String> {
    let mut highlights = Vec::new();
    for event in events.iter().rev() {
        if let Some(summary) = timeline_event_summary(event)
            && !highlights.contains(&summary)
        {
            highlights.push(summary);
        }
        if highlights.len() == 3 {
            break;
        }
    }
    highlights.reverse();
    highlights
}

fn timeline_event_summary(event: &Value) -> Option<String> {
    event
        .get("raw")
        .and_then(Value::as_str)
        .or_else(|| event.get("message").and_then(Value::as_str))
        .map(|text| text.chars().take(160).collect::<String>())
        .or_else(|| {
            event
                .get("parsed")
                .and_then(|parsed| parsed.get("phase"))
                .and_then(Value::as_str)
                .map(|phase| format!("lifecycle phase: {phase}"))
        })
}

pub(super) fn task_phase(status: TaskStatus) -> TaskPhase {
    match status {
        TaskStatus::Queued => TaskPhase::Pending,
        TaskStatus::Running => TaskPhase::Active,
        _ => TaskPhase::Done,
    }
}

/// Opt-in (`verbosity: "detailed"`) debug metadata added to lean responses.
/// Writes the process-outcome fields (exit code, signal, error, error type)
/// onto a task result object. No-op if `value` is not a JSON object.
pub(super) fn insert_outcome_fields(value: &mut Value, task: &TaskRecord) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    object.insert(
        "exitCode".to_string(),
        task.exit_code.map_or(Value::Null, Value::from),
    );
    object.insert(
        "signal".to_string(),
        task.signal.clone().map_or(Value::Null, Value::from),
    );
    object.insert(
        "error".to_string(),
        task.error.clone().map_or(Value::Null, Value::from),
    );
    object.insert("errorType".to_string(), json!(task.error_type));
}

/// Writes the review packet and, when the matching sections are requested, the
/// changed-files list and git status/diff. No-op if `value` is not an object.
pub(super) fn insert_evidence_fields(
    value: &mut Value,
    task: &TaskRecord,
    sections: &ResultSections,
    stdout_truncated: bool,
    stderr_truncated: bool,
) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    object.insert(
        "reviewPacket".to_string(),
        review_packet(task, stdout_truncated, stderr_truncated),
    );
    if sections.changed_files {
        object.insert(
            "changedFiles".to_string(),
            Value::Array(
                task.changed_files
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .map(Value::String)
                    .collect(),
            ),
        );
    }
    if sections.diff {
        object.insert(
            "gitStatus".to_string(),
            Value::String(task.git_status.clone().unwrap_or_default()),
        );
        object.insert(
            "gitDiff".to_string(),
            Value::String(task.git_diff.clone().unwrap_or_default()),
        );
    }
}

/// Writes the verbose diagnostic fields and delegates to `add_detail`. No-op if
/// `value` is not a JSON object.
pub(super) fn insert_detail_fields(value: &mut Value, task: &TaskRecord) {
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "diagnostic".to_string(),
            task.diagnostic.clone().unwrap_or(Value::Null),
        );
        object.insert(
            "transcriptAvailable".to_string(),
            Value::Bool(task.transcript_available),
        );
        object.insert(
            "finalResultDetected".to_string(),
            Value::Bool(task.final_result_detected),
        );
        object.insert(
            "partialResultDetected".to_string(),
            Value::Bool(task.partial_result_detected),
        );
    }
    add_detail(value, task);
}

pub(super) fn add_detail(value: &mut Value, task: &TaskRecord) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    object.insert("provider".to_string(), json!(task.provider));
    object.insert("mode".to_string(), json!(task.mode));
    object.insert("title".to_string(), json!(task.title));
    object.insert("cwd".to_string(), json!(task.cwd));
    object.insert("isolation".to_string(), json!(task.isolation));
    object.insert("worktreePath".to_string(), json!(task.worktree_path));
    object.insert("pid".to_string(), json!(task.pid));
    object.insert("createdAt".to_string(), json!(task.created_at));
    object.insert("updatedAt".to_string(), json!(task.updated_at));
    object.insert("startedAt".to_string(), json!(task.started_at));
    object.insert("completedAt".to_string(), json!(task.completed_at));
    object.insert("durationMs".to_string(), duration_ms(task));
    object.insert("errorType".to_string(), json!(task.error_type));
    object.insert("profile".to_string(), json!(task.profile));
    object.insert("promptStrategy".to_string(), json!(task.prompt_strategy));
    object.insert(
        "profileDiagnostics".to_string(),
        task.profile_diagnostics.clone().unwrap_or(Value::Null),
    );
    object.insert(
        "transcriptDiagnostic".to_string(),
        task.transcript_diagnostic.clone().unwrap_or(Value::Null),
    );
}

pub(super) fn display_title(task: &TaskRecord) -> String {
    task.title
        .clone()
        .unwrap_or_else(|| format!("{} {} task", task.provider.as_str(), task.mode.as_str()))
}

/// Single deduplicated `next` action list. Targets only the consolidated
/// eight-tool surface (agent_observe / agent_result / agent_stop / agent_remove).
pub(super) fn next_actions(task: &TaskRecord, progress: &Value) -> Value {
    let mut actions = Vec::new();
    if !is_final(task.status) {
        let recommended_poll_ms = progress["recommendedPollMs"].as_i64().unwrap_or(30_000);
        let stall_risk = progress["stallRisk"].as_str().unwrap_or("low");
        actions.push(next_action(
            "observe",
            Some("agent_observe"),
            json!({ "agentId": task.agent_id, "until": "now", "cursor": 0, "limit": 100, "timeoutMs": recommended_poll_ms }),
            "available",
            "Observe bounded transcript and lifecycle progress before deciding whether to wait, inspect, or stop.",
            "safe",
        ));
        actions.push(next_action(
            "wait_final",
            Some("agent_observe"),
            json!({ "agentId": task.agent_id, "until": "final", "timeoutMs": recommended_poll_ms.min(MAX_WAIT_MS) }),
            "available",
            "Block until the agent reaches a final state using the provider-aware polling interval.",
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
            "Inspect the review packet, then request stdout/stderr/diff/transcript sections as needed before cleanup or verification.",
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
            "inspect_evidence",
            Some("agent_result"),
            json!({ "agentId": task.agent_id, "sections": ["summary", "stdout", "stderr"] }),
            "available",
            "Inspect logs and diagnostics before deciding whether to rerun, narrow the prompt, or continue manually.",
            "safe",
        ));
        if task.transcript_available {
            actions.push(next_action(
                "inspect_transcript",
                Some("agent_result"),
                json!({ "agentId": task.agent_id, "sections": ["transcript"] }),
                "available",
                "Inspect transcript evidence when provider behavior or final-state classification is unclear.",
                "safe",
            ));
        }
        if !task.partial_results.is_empty() {
            let mut rerun_args = task.spawn_input.clone();
            if let Some(obj) = rerun_args.as_object_mut() {
                obj.remove("dryRun");
            }
            actions.push(next_action(
                "continue_rerun",
                Some("agent_spawn"),
                rerun_args,
                "available",
                "Partial results were collected before the task ended. Consider rerunning with the same or narrowed parameters.",
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

pub(super) fn completion_notification_params(task: &TaskRecord) -> Value {
    let progress = agent_progress(task);
    let changed_files = task.changed_files.clone().unwrap_or_default();
    let changed_file_count = changed_files.len();
    let git_status = task.git_status.clone().unwrap_or_default();
    let has_changes = !changed_files.is_empty() || !git_status.trim().is_empty();

    json!({
        "agentId": task.agent_id,
        "provider": task.provider,
        "mode": task.mode,
        "title": task.title,
        "displayTitle": display_title(task),
        "status": task.status,
        "isFinal": is_final(task.status),
        "phase": task_phase(task.status),
        "completedAt": task.completed_at,
        "attentionRequired": task.result_inspected_at.is_none(),
        "summary": {
            "exitCode": task.exit_code,
            "signal": task.signal,
            "errorType": task.error_type,
            "hasChanges": has_changes,
            "changedFiles": changed_files,
            "changedFileCount": changed_file_count,
            "transcriptAvailable": task.transcript_available,
            "finalResultDetected": task.final_result_detected,
            "partialResultDetected": task.partial_result_detected,
            "recommendedActions": recommended_actions(task, has_changes),
            "next": next_actions(task, &progress),
        }
    })
}

pub(super) fn next_action(
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

pub(super) fn review_packet(
    task: &TaskRecord,
    stdout_truncated: bool,
    stderr_truncated: bool,
) -> Value {
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
        "partialResults": task.partial_results,
        "recommendedActions": recommended_actions(task, has_changes)
    })
}

pub(super) fn recommended_actions(task: &TaskRecord, has_changes: bool) -> Vec<&'static str> {
    if !is_final(task.status) {
        return vec![
            "Use agent_observe with a bounded timeout before treating silence as a stall.",
            "Use agent_observe with limit:0 to confirm whether the agent is still active.",
            "Use agent_observe with until:final when only finality matters.",
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
        actions.push("Request agent_result sections:[\"transcript\"] when provider behavior or final-state classification is unclear.");
    }
    if has_changes {
        actions.push("Inspect gitStatus, gitDiff, and changedFiles before verification.");
    }
    if !task.partial_results.is_empty() {
        actions.push("Partial results were detected: inspect them and consider rerunning or continuing the task.");
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

pub(super) fn is_final(status: TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Succeeded | TaskStatus::Failed | TaskStatus::Stopped | TaskStatus::FailedStale
    )
}

pub(super) fn transition_status(task: &mut TaskRecord, next: TaskStatus) -> Result<(), String> {
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

pub(super) fn duration_ms(task: &TaskRecord) -> Value {
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

pub(super) struct CappedText {
    pub text: String,
    pub truncated: bool,
}

pub(super) async fn read_capped_file(path: &Path, max_bytes: usize) -> Result<CappedText, String> {
    match fs::File::open(path).await {
        Ok(file) => {
            let mut reader = file.take(max_bytes.saturating_add(1) as u64);
            let mut bytes = Vec::new();
            reader
                .read_to_end(&mut bytes)
                .await
                .map_err(|error| error.to_string())?;
            let truncated = bytes.len() > max_bytes;
            if truncated {
                bytes.truncate(max_bytes);
            }
            Ok(CappedText {
                text: String::from_utf8_lossy(&bytes).to_string(),
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

pub(super) struct SlicedLines {
    pub text: String,
    pub next_line: usize,
}

pub(super) fn slice_lines(text: &str, start_line: usize) -> SlicedLines {
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

pub(super) fn normalize_wait_ms(value: Option<i64>) -> i64 {
    value.unwrap_or(30_000).clamp(0, MAX_WAIT_MS)
}

pub(super) fn normalize_observe_ms(value: Option<i64>) -> i64 {
    value.unwrap_or(30_000).clamp(0, MAX_OBSERVE_MS)
}

pub(super) fn normalize_observe_limit(value: Option<u64>) -> usize {
    value.unwrap_or(100).clamp(1, MAX_OBSERVE_EVENTS as u64) as usize
}

pub(super) fn normalize_max_bytes(value: Option<i64>) -> usize {
    value
        .unwrap_or(MAX_LOG_BYTES as i64)
        .clamp(1, MAX_LOG_BYTES as i64) as usize
}
