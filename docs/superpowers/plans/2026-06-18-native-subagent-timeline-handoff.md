# Native Subagent Timeline And Handoff Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add default `timeline` and `handoff` response blocks so Agent Bridge agents feel like managed host-agent subagents during progress and result consumption.

**Architecture:** Keep the existing eight-tool MCP lifecycle. Add two small shaping helpers in `task/review.rs`, wire them into existing observe/status/result payloads, and update schemas/tests so callers can rely on the new blocks without fetching raw evidence.

**Tech Stack:** Rust 2024, Tokio, serde_json, existing fake-provider stdio integration tests, `scripts/quality.sh`.

## Global Constraints

- No ninth MCP lifecycle tool.
- No new task storage format.
- No live-provider dependency for tests.
- No claim that provider success means local verification passed.
- No provider-specific transcript parser beyond existing normalized lifecycle and transcript data.
- Raw stdout, stderr, transcript, and diff bodies remain opt-in through existing `agent_result.sections`.
- `verificationStatus` is always `not_verified`.

---

## File Structure

- Modify: `crates/agent-bridge-mcp/src/task/review.rs`
  - Add `agent_timeline`, `agent_handoff`, and tiny private helpers for state/outcome/highlights.
  - Keep helpers pure and derived from `TaskRecord`, `progress`, transcript events, and `reviewPacket`.
- Modify: `crates/agent-bridge-mcp/src/task.rs`
  - Re-export new helper names from the private `review` module only where existing result assembly needs them.
  - Add focused unit tests in the existing `#[cfg(test)]` module.
- Modify: `crates/agent-bridge-mcp/src/tools.rs`
  - Add `timeline` and `handoff` to output schemas.
- Modify: `crates/agent-bridge-mcp/tests/server_protocol.rs`
  - Assert schemas advertise the new fields.
- Modify: `crates/agent-bridge-mcp/tests/stdio_binary.rs`
  - Extend existing fake-provider integration tests for observe/result behavior.

---

### Task 1: Observe Timeline

**Files:**
- Modify: `crates/agent-bridge-mcp/src/task/review.rs`
- Modify: `crates/agent-bridge-mcp/src/task.rs`

**Interfaces:**
- Consumes: `TaskRecord`, `agent_progress(task) -> Value`, `next_actions(task, &progress) -> Value`, transcript event `Value`s.
- Produces: `pub(super) fn agent_timeline(task: &TaskRecord, events: &[Value], progress: &Value) -> Value`.
- Produces: `public_task(task)` and `observe_payload(task, transcript, timed_out, detailed)` include `timeline`.

- [ ] **Step 1: Write failing unit tests**

Add these tests to the existing `#[cfg(test)] mod tests` in `crates/agent-bridge-mcp/src/task.rs` near the other `public_task` and `next_actions` tests:

```rust
#[test]
fn agent_timeline_marks_running_activity_from_events() {
    let task = sample_task(TaskStatus::Running);
    let progress = agent_progress(&task);
    let events = vec![
        json!({"kind": "lifecycle", "parsed": {"phase": "spawned"}}),
        json!({"kind": "provider_event", "raw": "provider is reviewing files"}),
    ];

    let timeline = agent_timeline(&task, &events, &progress);

    assert_eq!(timeline["state"], "working");
    assert_eq!(timeline["attention"], "wait");
    assert_eq!(timeline["next"][0]["id"], "observe");
    assert!(
        timeline["headline"]
            .as_str()
            .unwrap()
            .contains("codex review task")
    );
    assert!(
        timeline["recentHighlights"]
            .as_array()
            .unwrap()
            .iter()
            .any(|highlight| highlight
                .as_str()
                .unwrap()
                .contains("provider is reviewing files"))
    );
}

#[test]
fn public_task_includes_quiet_timeline_for_state_only_observe_paths() {
    let task = sample_task(TaskStatus::Running);

    let public = public_task(&task);

    assert_eq!(public["timeline"]["state"], "quiet");
    assert_eq!(public["timeline"]["attention"], "wait");
    assert!(public["timeline"]["recentHighlights"].as_array().unwrap().is_empty());
    assert_eq!(public["timeline"]["next"][0]["id"], "observe");
}

#[test]
fn agent_timeline_marks_high_stall_risk_as_stalled() {
    let task = sample_task(TaskStatus::Running);
    let progress = json!({
        "stallRisk": "high",
        "recommendedPollMs": 30000
    });

    let timeline = agent_timeline(&task, &[], &progress);

    assert_eq!(timeline["state"], "stalled");
    assert_eq!(timeline["attention"], "inspect");
    assert!(
        timeline["headline"]
            .as_str()
            .unwrap()
            .contains("needs attention")
    );
}
```

Update the private import list in `crates/agent-bridge-mcp/src/task.rs` so tests can call the helper:

```rust
use review::{
    AGENT_COMPLETED_NOTIFICATION, add_detail, agent_timeline, completion_notification_params,
    display_title, insert_detail_fields, insert_evidence_fields, insert_outcome_fields, is_final,
    list_tasks, normalize_max_bytes, normalize_observe_limit, normalize_observe_ms,
    normalize_wait_ms, observe_payload, public_task, read_capped_file, read_transcript,
    slice_lines, transcript_evidence, transition_status,
};
```

- [ ] **Step 2: Run the focused failing tests**

Run:

```bash
cargo test -p agent-bridge-mcp agent_timeline -- --test-threads=1
```

Expected: FAIL because `agent_timeline` does not exist.

- [ ] **Step 3: Implement the minimal timeline helper and wire it into observe payloads**

In `crates/agent-bridge-mcp/src/task/review.rs`, add these helpers after `public_task` or near the other response-shaping helpers:

```rust
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
```

Update `public_task`:

```rust
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
```

Update `observe_payload`:

```rust
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
```

- [ ] **Step 4: Run focused timeline tests**

Run:

```bash
cargo test -p agent-bridge-mcp agent_timeline -- --test-threads=1
cargo test -p agent-bridge-mcp public_task_includes_quiet_timeline_for_state_only_observe_paths -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-bridge-mcp/src/task.rs crates/agent-bridge-mcp/src/task/review.rs
git commit -m "feat: add agent observe timeline"
```

---

### Task 2: Result Handoff

**Files:**
- Modify: `crates/agent-bridge-mcp/src/task/review.rs`
- Modify: `crates/agent-bridge-mcp/src/task.rs`

**Interfaces:**
- Consumes: `review_packet(task, stdout_truncated, stderr_truncated) -> Value`, `next_actions(task, &progress) -> Value`.
- Produces: `pub(super) fn agent_handoff(task: &TaskRecord, review_packet: &Value) -> Value`.
- Produces: `agent_result` default payload includes `handoff`.

- [ ] **Step 1: Write failing unit tests**

Add these tests to the existing test module in `crates/agent-bridge-mcp/src/task.rs` near `insert_evidence_fields_respects_section_flags`:

```rust
#[test]
fn agent_handoff_reports_success_without_verification_claims() {
    let mut task = sample_task(TaskStatus::Succeeded);
    task.final_result_detected = true;
    task.changed_files = Some(vec!["README.md".to_string()]);
    let review = review_packet(&task, false, false);

    let handoff = agent_handoff(&task, &review);

    assert_eq!(handoff["outcome"], "succeeded");
    assert_eq!(handoff["verificationStatus"], "not_verified");
    assert_eq!(handoff["changedFiles"]["count"], 1);
    assert_eq!(handoff["changedFiles"]["paths"], json!(["README.md"]));
    assert_eq!(handoff["next"][0]["id"], "inspect_result");
    assert!(
        handoff["summary"]
            .as_str()
            .unwrap()
            .contains("finished successfully")
    );
}

#[test]
fn agent_handoff_reports_partial_before_failed_outcome() {
    let mut task = sample_task(TaskStatus::Failed);
    task.partial_result_detected = true;
    task.final_result_detected = false;
    task.error_type = Some(ErrorType::Timeout);
    let review = review_packet(&task, false, false);

    let handoff = agent_handoff(&task, &review);

    assert_eq!(handoff["outcome"], "partial");
    assert_eq!(handoff["verificationStatus"], "not_verified");
    assert!(
        handoff["summary"]
            .as_str()
            .unwrap()
            .contains("partial evidence")
    );
    assert_eq!(handoff["evidenceRefs"], json!(["summary", "stdout", "stderr", "transcript"]));
}

#[test]
fn agent_handoff_reports_failed_stopped_and_stale_outcomes() {
    for (status, error_type, outcome) in [
        (TaskStatus::Failed, Some(ErrorType::ProviderOutputError), "failed"),
        (TaskStatus::Stopped, None, "stopped"),
        (TaskStatus::FailedStale, Some(ErrorType::Stale), "stale"),
    ] {
        let mut task = sample_task(status);
        task.error_type = error_type;
        let review = review_packet(&task, false, false);

        let handoff = agent_handoff(&task, &review);

        assert_eq!(handoff["outcome"], outcome);
        assert_eq!(handoff["verificationStatus"], "not_verified");
        assert!(handoff["evidenceRefs"].as_array().unwrap().contains(&json!("summary")));
    }
}
```

Update the private import list in `crates/agent-bridge-mcp/src/task.rs`:

```rust
use review::{
    AGENT_COMPLETED_NOTIFICATION, add_detail, agent_handoff, agent_timeline,
    completion_notification_params, display_title, insert_detail_fields, insert_evidence_fields,
    insert_outcome_fields, is_final, list_tasks, normalize_max_bytes, normalize_observe_limit,
    normalize_observe_ms, normalize_wait_ms, observe_payload, public_task, read_capped_file,
    read_transcript, review_packet, slice_lines, transcript_evidence, transition_status,
};
```

- [ ] **Step 2: Run focused failing tests**

Run:

```bash
cargo test -p agent-bridge-mcp agent_handoff -- --test-threads=1
```

Expected: FAIL because `agent_handoff` does not exist.

- [ ] **Step 3: Implement the minimal handoff helper and wire it into result payloads**

In `crates/agent-bridge-mcp/src/task/review.rs`, add:

```rust
pub(super) fn agent_handoff(task: &TaskRecord, review_packet: &Value) -> Value {
    let changed_files = task.changed_files.clone().unwrap_or_default();
    let progress = agent_progress(task);
    json!({
        "outcome": handoff_outcome(task),
        "summary": handoff_summary(task),
        "changedFiles": {
            "count": changed_files.len(),
            "paths": changed_files.into_iter().take(25).collect::<Vec<_>>()
        },
        "verificationStatus": "not_verified",
        "evidenceRefs": handoff_evidence_refs(task),
        "errorType": task.error_type,
        "reviewPacket": {
            "status": review_packet["status"].clone(),
            "phase": review_packet["phase"].clone(),
            "transcriptAvailable": review_packet["transcriptAvailable"].clone(),
            "finalResultDetected": review_packet["finalResultDetected"].clone(),
            "partialResultDetected": review_packet["partialResultDetected"].clone()
        },
        "next": next_actions(task, &progress)
    })
}

fn handoff_outcome(task: &TaskRecord) -> &'static str {
    if task.partial_result_detected && !task.final_result_detected {
        return "partial";
    }
    match task.status {
        TaskStatus::Succeeded => "succeeded",
        TaskStatus::Failed => "failed",
        TaskStatus::Stopped => "stopped",
        TaskStatus::FailedStale => "stale",
        TaskStatus::Queued | TaskStatus::Running => "partial",
    }
}

fn handoff_summary(task: &TaskRecord) -> String {
    let title = display_title(task);
    if task.partial_result_detected && !task.final_result_detected {
        return format!("{title} ended with partial evidence but no trusted final result.");
    }
    match task.status {
        TaskStatus::Succeeded => format!("{title} finished successfully; verify locally before claiming completion."),
        TaskStatus::Failed => format!("{title} failed; inspect evidence before retrying."),
        TaskStatus::Stopped => format!("{title} was stopped; inspect evidence before deciding whether to continue."),
        TaskStatus::FailedStale => format!("{title} is stale; inspect evidence before rerunning."),
        TaskStatus::Queued | TaskStatus::Running => format!("{title} has not reached a final result."),
    }
}

fn handoff_evidence_refs(task: &TaskRecord) -> Vec<&'static str> {
    let mut refs = vec!["summary"];
    if task.changed_files.as_ref().is_some_and(|files| !files.is_empty()) {
        refs.push("changedFiles");
        refs.push("diff");
    }
    if task.error_type.is_some() || matches!(task.status, TaskStatus::Failed | TaskStatus::FailedStale) {
        refs.push("stdout");
        refs.push("stderr");
    }
    if task.transcript_available || task.partial_result_detected {
        refs.push("transcript");
    }
    refs
}
```

In `crates/agent-bridge-mcp/src/task.rs`, update `TaskManagerHandle::result` after `insert_evidence_fields(...)`:

```rust
insert_evidence_fields(
    &mut value,
    &task,
    &sections,
    stdout_truncated,
    stderr_truncated,
);
let review_packet = value["reviewPacket"].clone();
if let Some(object) = value.as_object_mut() {
    object.insert("handoff".to_string(), agent_handoff(&task, &review_packet));
}
```

- [ ] **Step 4: Run focused handoff tests**

Run:

```bash
cargo test -p agent-bridge-mcp agent_handoff -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-bridge-mcp/src/task.rs crates/agent-bridge-mcp/src/task/review.rs
git commit -m "feat: add agent result handoff"
```

---

### Task 3: Schemas And Stdio Integration

**Files:**
- Modify: `crates/agent-bridge-mcp/src/tools.rs`
- Modify: `crates/agent-bridge-mcp/tests/server_protocol.rs`
- Modify: `crates/agent-bridge-mcp/tests/stdio_binary.rs`

**Interfaces:**
- Consumes: `agent_observe.timeline` from Task 1 and `agent_result.handoff` from Task 2.
- Produces: MCP output schemas advertise `timeline` and `handoff`.
- Produces: fake-provider tests prove the default stdio behavior without live providers.

- [ ] **Step 1: Write failing schema and integration assertions**

In `crates/agent-bridge-mcp/tests/server_protocol.rs`, extend `consolidated_agent_read_schemas_expose_lean_next_list` with:

```rust
let observe_output = tools
    .iter()
    .find(|tool| tool["name"] == "agent_observe")
    .unwrap()["outputSchema"]
    .clone();
assert_eq!(
    observe_output["properties"]["timeline"]["type"],
    "object",
    "agent_observe should advertise timeline"
);

let result_output = tools
    .iter()
    .find(|tool| tool["name"] == "agent_result")
    .unwrap()["outputSchema"]
    .clone();
assert_eq!(
    result_output["properties"]["handoff"]["type"],
    "object",
    "agent_result should advertise handoff"
);
assert!(
    result_output["required"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("handoff"))
);
```

In `crates/agent-bridge-mcp/tests/stdio_binary.rs`, extend `stdio_agent_observe_returns_events_and_progress`:

```rust
assert!(observed["timeline"].is_object());
assert!(["working", "final"].contains(&observed["timeline"]["state"].as_str().unwrap()));
assert!(observed["timeline"]["headline"].as_str().unwrap().contains("codex review task"));
assert!(observed["timeline"]["next"].is_array());
assert!(observed["timeline"]["recentHighlights"].is_array());
```

Extend `stdio_agent_observe_timeout_does_not_fail_running_agent`:

```rust
assert_eq!(second["timeline"]["state"], "quiet");
assert_eq!(second["timeline"]["attention"], "wait");
```

Extend `stdio_agent_result_review_packet_summarizes_worktree_changes`:

```rust
assert_eq!(result["handoff"]["outcome"], "succeeded");
assert_eq!(result["handoff"]["verificationStatus"], "not_verified");
assert_eq!(result["handoff"]["changedFiles"]["count"], 1);
assert_eq!(result["handoff"]["changedFiles"]["paths"], json!(["README.md"]));
assert!(
    result["handoff"]["evidenceRefs"]
        .as_array()
        .unwrap()
        .contains(&json!("diff"))
);
```

Extend `stdio_agent_result_preserves_final_result_evidence_after_timeout`:

```rust
assert_eq!(result["handoff"]["outcome"], "partial");
assert_eq!(result["handoff"]["verificationStatus"], "not_verified");
assert!(
    result["handoff"]["evidenceRefs"]
        .as_array()
        .unwrap()
        .contains(&json!("transcript"))
);
```

Extend `stdio_claude_agent_malformed_output_returns_diagnostic`:

```rust
assert_eq!(result["handoff"]["outcome"], "failed");
assert_eq!(result["handoff"]["verificationStatus"], "not_verified");
assert!(
    result["handoff"]["evidenceRefs"]
        .as_array()
        .unwrap()
        .contains(&json!("stderr"))
);
```

- [ ] **Step 2: Run focused failing tests**

Run:

```bash
cargo test -p agent-bridge-mcp consolidated_agent_read_schemas_expose_lean_next_list -- --test-threads=1
cargo test -p agent-bridge-mcp --test stdio_binary stdio_agent_observe_returns_events_and_progress -- --test-threads=1
cargo test -p agent-bridge-mcp --test stdio_binary stdio_agent_result_review_packet_summarizes_worktree_changes -- --test-threads=1
cargo test -p agent-bridge-mcp --test stdio_binary stdio_agent_result_preserves_final_result_evidence_after_timeout -- --test-threads=1
cargo test -p agent-bridge-mcp --test stdio_binary stdio_claude_agent_malformed_output_returns_diagnostic -- --test-threads=1
```

Expected: schema test fails until `tools.rs` changes; integration tests pass after Tasks 1 and 2 except schema coverage.

- [ ] **Step 3: Update output schemas**

In `crates/agent-bridge-mcp/src/tools.rs`, update `output_schema_for("agent_observe")`:

```rust
"agent_observe" => output_object_schema(
    json!({
        "agentId": {"type": "string"},
        "status": {"type": "string"},
        "isFinal": {"type": "boolean"},
        "phase": {"type": "string"},
        "progress": {"type": "object"},
        "timeline": {"type": "object"},
        "events": {"type": "array"},
        "nextCursor": {"type": "integer"},
        "timedOut": {"type": "boolean"},
        "next": {"type": "array"}
    }),
    vec!["agentId", "status", "isFinal", "phase", "progress", "timeline", "next"],
),
```

Update `output_schema_for("agent_result")`:

```rust
"agent_result" => output_object_schema(
    json!({
        "agentId": {"type": "string"},
        "status": {"type": "string"},
        "isFinal": {"type": "boolean"},
        "phase": {"type": "string"},
        "reviewPacket": {"type": "object"},
        "handoff": {"type": "object"},
        "changedFiles": {"type": "array"},
        "next": {"type": "array"}
    }),
    vec!["agentId", "status", "isFinal", "reviewPacket", "handoff", "next"],
),
```

- [ ] **Step 4: Run focused schema and integration tests**

Run:

```bash
cargo test -p agent-bridge-mcp consolidated_agent_read_schemas_expose_lean_next_list -- --test-threads=1
cargo test -p agent-bridge-mcp --test stdio_binary stdio_agent_observe_returns_events_and_progress -- --test-threads=1
cargo test -p agent-bridge-mcp --test stdio_binary stdio_agent_observe_timeout_does_not_fail_running_agent -- --test-threads=1
cargo test -p agent-bridge-mcp --test stdio_binary stdio_agent_result_review_packet_summarizes_worktree_changes -- --test-threads=1
cargo test -p agent-bridge-mcp --test stdio_binary stdio_agent_result_preserves_final_result_evidence_after_timeout -- --test-threads=1
cargo test -p agent-bridge-mcp --test stdio_binary stdio_claude_agent_malformed_output_returns_diagnostic -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 5: Run full verification**

Run:

```bash
cargo fmt --check
cargo test -p agent-bridge-mcp -- --test-threads=1
cargo test -p agent-bridge-mcp --test stdio_binary -- --test-threads=1
git diff --check
scripts/quality.sh
```

Expected: all commands exit 0. If `scripts/quality.sh` reports only informational jscpd/complexity/module-graph output and exits 0, treat it as pass.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-bridge-mcp/src/tools.rs crates/agent-bridge-mcp/tests/server_protocol.rs crates/agent-bridge-mcp/tests/stdio_binary.rs
git commit -m "test: cover native subagent payloads"
```

---

## Final Verification

After all tasks are committed, run:

```bash
git status --short
scripts/quality.sh
```

Expected:

- `git status --short` is clean.
- `scripts/quality.sh` exits 0.

Do not push unless explicitly asked.
