# Native Subagent Timeline And Handoff Design

## Goal

Make Agent Bridge agents feel more like managed host-agent subagents while
preserving the existing eight-tool MCP lifecycle and the caller-owned
verification boundary.

The target friction is progress and result consumption:

- Running agents should read like a concise subagent timeline, not a raw task
  registry update.
- Finished agents should report back through a compact handoff packet before
  callers fetch raw evidence.

## Non-Goals

- No ninth MCP lifecycle tool.
- No new task storage format.
- No live-provider dependency for tests.
- No claim that provider success means local verification passed.
- No provider-specific transcript parser beyond existing normalized lifecycle
  and transcript data.

## Approach

Keep the public lifecycle tools unchanged. Add display-friendly response blocks
to the existing result shapes:

- `agent_observe` includes a default `timeline` block.
- `agent_result` includes a default `handoff` block.

Raw stdout, stderr, transcript, and diff bodies remain opt-in through existing
`agent_result.sections`.

## `agent_observe.timeline`

`timeline` is derived from existing task state, progress, lifecycle events, and
bounded transcript events.

Fields:

- `headline`: one short status sentence.
- `state`: one of `queued`, `working`, `quiet`, `stalled`, or `final`.
- `currentActivity`: best available normalized activity, or `null`.
- `recentHighlights`: bounded, deduped lifecycle/transcript highlights.
- `attention`: one of `wait`, `inspect`, `read_result`, or `stop`.
- `next`: the existing structured action list.

State rules:

- `queued` for queued tasks.
- `working` when recent lifecycle or transcript activity exists.
- `quiet` when the task is running but does not meet the existing high-risk
  stall criteria.
- `stalled` only when existing progress/stall-risk logic marks the task as high
  risk.
- `final` for final statuses.

## `agent_result.handoff`

`handoff` is derived from the existing review packet, result detection flags,
changed files, and diagnostics.

Fields:

- `outcome`: one of `succeeded`, `failed`, `stopped`, `stale`, or `partial`.
- `summary`: compact final report suitable for the host agent to read.
- `changedFiles`: count plus bounded path list.
- `verificationStatus`: always `not_verified`.
- `evidenceRefs`: exact existing result sections to request for raw evidence.
- `next`: existing structured actions.

Outcome rules:

- `partial` when there is partial/provider output but no trusted final result.
- `failed` for failed task statuses.
- `stopped` for stopped tasks.
- `stale` for failed-stale tasks.
- `succeeded` for successful final tasks with a trusted final provider result.

## Data Flow

Add two small shaping helpers in `crates/agent-bridge-mcp/src/task/review.rs`:

- `agent_timeline(task, events, progress) -> Value`
- `agent_handoff(task, review_packet) -> Value`

The helpers use only existing fields:

- task status, timestamps, provider, mode, title, profile, diagnostics
- existing transcript/lifecycle events
- existing `progress`, `partialResults`, `reviewPacket`, and `next_actions`
- existing final and partial result detection flags

No new persistence is required. If transcript events are unavailable, the
timeline falls back to lifecycle events and task status.

## Error Handling And Boundaries

- Final but uninspected tasks set `timeline.state = "final"` and
  `timeline.attention = "read_result"`.
- Failed handoffs include `errorType` and a short diagnostic without embedding
  logs.
- Partial handoffs state that partial evidence exists and avoid completion
  claims.
- Changed files are listed by path and count only; diffs remain opt-in.
- `verificationStatus` is always `not_verified`.
- Cleanup stays unsafe until final result inspection for managed worktrees.

## Testing

Use deterministic fake-provider and registry tests only:

- running observe response includes a timeline with highlights and next actions
- quiet running task reports `quiet`, not `stalled`
- high-risk running task reports `stalled`
- successful final result includes a handoff with `not_verified`
- partial result includes `outcome: "partial"`
- failed result includes failure outcome and evidence refs

Run the smallest focused test first, then the project quality gate before
implementation is considered done.
