## Context

Agent Bridge is a Rust stdio MCP server that delegates work to Claude, Cursor, Kimi, and Codex through a provider-neutral task lifecycle. The current lifecycle is operationally strong: callers can preview, spawn, wait, inspect logs, inspect transcripts, inspect final results, stop tasks, and remove completed task records. Task records also preserve provider, mode, title, timestamps, diagnostics, git state, transcript evidence, and review packets.

The client experience still differs from native subagents. Native subagents are rendered as visible agent entities with compact lifecycle controls. Agent Bridge tasks are rendered as MCP tool calls and task IDs, so the caller/model must know the orchestration recipe and manually choose which lifecycle tool to call next. The change should make Agent Bridge tasks easy for clients to render as native-feeling agents without weakening the verification boundary: provider output remains evidence, and the main caller remains responsible for verification.

## Goals / Non-Goals

**Goals:**

- Provide a compact, stable presentation model for Agent Bridge tasks as client-renderable agents.
- Make active/recent task discovery practical for UI lists without returning the full historical registry by default.
- Surface action availability explicitly so clients can render enabled controls and disabled controls with reasons.
- Align runtime provider metadata with the capabilities needed by the client, including launch profiles and reply/resume support.
- Preserve lower-level lifecycle tools for automation and debugging.

**Non-Goals:**

- Do not build a separate web UI in this change.
- Do not require provider CLIs to support true interactive sessions when they do not.
- Do not treat provider success, transcript final-result detection, or review packets as project verification.
- Do not remove existing task lifecycle tools or change their current core behavior.
- Do not add a new third-party dependency.

## Decisions

### Decision: Add `presentation` to task lifecycle tools instead of adding `agent_*` tools in v1

Agent Bridge should expose a compact native-client view as a nested `presentation` object derived from existing task records. The v1 API should keep `task_*` as the public MCP noun and add optional filters to `task_list`; it should not add separate `agent_*` tools unless a later host integration proves that separate discovery is needed. The source of truth remains the task registry and task result artifacts.

Rationale: Native UX needs display-friendly summaries, but existing tools are already useful for automation, debugging, and safe inspection. A derived presentation contract avoids duplicating task state.

Alternatives considered:

- Replace the task API with `agent_*` tools. Rejected for v1 because it would duplicate lifecycle concepts, churn tool-list tests, and make the API less consistent with the current provider-neutral task model.
- Make the client infer presentation from raw task fields. Rejected because every client would need to duplicate task-state and action-availability logic.

### Decision: Represent unsupported actions explicitly

Task presentation summaries should include action availability for common controls such as wait, inspect result, inspect logs, inspect transcript, stop, cleanup, reply, and resume. Unsupported actions should be exposed as unavailable with a reason rather than omitted.

Rationale: Native clients need predictable controls. Explicit unavailable actions let the UI show that Agent Bridge does not currently support reply/resume for a provider rather than making that absence look like a missing integration.

Alternatives considered:

- Hide unsupported actions. Rejected because users cannot tell whether a control is unsupported, temporarily unavailable, or missing due to a bug.
- Pretend reply/resume exists through respawning. Rejected because that would create misleading conversational semantics over a batch task model.

### Decision: Keep native-feeling UX additive and host-neutral

The MCP server should expose enough structured metadata for any host to render Agent Bridge tasks well, without depending on Codex-specific UI APIs. Host-specific native integration can map the contract into its own UI, but the MCP surface remains the portable boundary.

Rationale: Agent Bridge is an MCP server. A host-neutral contract keeps the feature useful across Codex and other MCP clients while still giving Codex enough structure for native-quality rendering.

Alternatives considered:

- Add Codex-only metadata. Rejected for the first slice because it would make core behavior harder to test and less portable.
- Rely only on MCP prompts/resources. Rejected because prompts help model behavior but do not give the client a stable UI data model.

### Decision: Filter active/recent task presentation before broad history

The default `task_list` response should be the client-facing presentation list: active tasks first, then recent final tasks sorted by `updatedAt` descending. It should default to a bounded limit of 25 summaries and support filters for status, provider, mode, workspace/cwd, title text, and limit. Full history can remain available intentionally through an explicit raw lifecycle inspection path.

Rationale: The current registry can contain many historical tasks across workspaces. Native UI should not make users or models scan the entire registry to find the relevant agent.

Alternatives considered:

- Keep `task_list` as the only list. Rejected because it is operationally complete but too noisy for a native task drawer.
- Auto-prune old tasks. Rejected for this change because cleanup can delete inspectable worktree state and should remain intentional.

## Risks / Trade-offs

- Presentation fields may drift from lifecycle fields -> derive them from existing task records and cover derivation with tests.
- Client UX may imply provider output is verified -> keep review packet and guidance language explicit that provider output is evidence only.
- Reply/resume controls may confuse users when unavailable -> return explicit unavailable reasons and provider capability metadata.
- Filter/list additions change `task_list` defaults -> keep raw full-history inspection explicit with `presentation: false` and `scope: "all"`.
- Runtime schema may lag source if an old binary is installed -> add tests against the production binary/tool schema and document upgrade expectations.

## Migration Plan

1. Add presentation metadata and list/filter behavior behind additive response fields and optional `task_list` arguments.
2. Update provider capability reporting so runtime metadata exposes launch profiles and action capabilities consistently.
3. Update guidance and README to show the native-client workflow and the raw lifecycle workflow.
4. Add deterministic tests using fake providers and stdio fixtures; do not require live provider execution.
5. Keep existing callers compatible by preserving current task lifecycle tools and fields.

Rollback is straightforward because the change is additive: clients can ignore the new fields/tools and continue using the existing lifecycle surface.

## Contract Sketch

`task_list`, `task_status`, and `task_result` should include a `presentation` object derived from the task record. `task_result.reviewPacket.recommendedActions` remains prose guidance for operators and models; `presentation.actions` is the structured UI affordance model.

Suggested `presentation` shape:

```json
{
  "displayTitle": "Review native Agent Bridge UX OpenSpec proposal",
  "subtitle": "cursor review",
  "phase": "active",
  "statusTone": "running",
  "workspace": "/Users/pedro/Development/agent-bridge-mcp",
  "timestamps": {
    "createdAt": "...",
    "updatedAt": "...",
    "startedAt": "...",
    "completedAt": null
  },
  "durationMs": 49979,
  "result": {
    "available": false,
    "hasChanges": false,
    "changedFileCount": 0,
    "transcriptAvailable": true,
    "finalResultDetected": false,
    "partialResultDetected": true
  },
  "verificationStatus": "not_verified",
  "actions": [
    {
      "id": "wait",
      "tool": "task_wait",
      "state": "available"
    },
    {
      "id": "reply",
      "tool": null,
      "state": "unavailable",
      "reason": "provider_task_not_interactive"
    }
  ]
}
```

`displayTitle` uses `TaskRecord.title` when present. If the title is absent, it falls back to `<provider> <mode> task`; the original prompt is not used as a display-title fallback in compact summaries.

Action state values:

- `available`: the client can call the mapped tool immediately.
- `unavailable`: the action is not supported for this provider/task state.
- `unsafe`: the action exists but should not be presented as a default/safe click; use for cleanup before final result inspection of managed worktree tasks.

Action mapping:

| Action | Tool | Available when |
| --- | --- | --- |
| `wait` | `task_wait` | task is not final |
| `inspect_status` | `task_status` | always |
| `inspect_logs` | `task_logs` | task is inspectable; removed tasks are excluded from presentation lists before action rendering |
| `inspect_transcript` | `task_transcript` | transcript is available |
| `inspect_result` | `task_result` | task is final |
| `stop` | `task_stop` | task is running or queued |
| `cleanup` | `task_remove` | task is final; state is `unsafe` for managed worktree tasks until final result inspection is explicit |
| `reply` | none in v1 | unavailable with `provider_task_not_interactive` |
| `resume` | none in v1 | unavailable with `provider_task_not_resumable` |

`task_list` filter sketch:

```json
{
  "presentation": true,
  "scope": "active_recent",
  "status": ["running", "queued"],
  "provider": ["cursor"],
  "mode": ["review"],
  "cwd": "/Users/pedro/Development/agent-bridge-mcp",
  "titleContains": "UX",
  "limit": 25
}
```

Default `task_list` presentation ordering is active tasks first, then final tasks by `updatedAt` descending. Default `limit` is 25; maximum accepted `limit` is 100. Removed tasks remain excluded from default presentation lists. Raw full-history inspection is explicit with `presentation: false` and `scope: "all"`.

## Open Questions

- Can any provider support true reply/resume in a follow-up change, or should interactive continuation remain explicitly out of scope beyond v1?
