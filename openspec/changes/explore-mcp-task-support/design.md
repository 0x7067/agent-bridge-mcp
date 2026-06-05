## Context

Agent Bridge has a mature custom task lifecycle exposed as `task_spawn`, `task_list`, `task_status`, `task_wait`, `task_logs`, `task_transcript`, `task_result`, `task_stop`, and `task_remove`. The current MCP specification includes experimental protocol task support for durable request state, polling, cancellation, progress, input-required states, TTL, and deferred result retrieval.

The research found an important compatibility constraint: the 2025-11-25 experimental task surface is not guaranteed to be stable, and later task-extension work signals wire-level migration differences. Agent Bridge should not replace its stable lifecycle with an experimental protocol surface until host support and negotiated semantics are proven.

## Goals / Non-Goals

**Goals:**

- Produce a compatibility design for mapping Agent Bridge task records to MCP task concepts.
- Implement only a negotiated, additive protocol task slice after compatibility is explicit.
- Preserve existing `task_*` tools as the stable lifecycle API.
- Cover clients that do and do not advertise protocol task support.
- Keep task result verification and managed-worktree cleanup boundaries intact.

**Non-Goals:**

- Do not remove or rename existing Agent Bridge lifecycle tools.
- Do not require MCP task support for normal Agent Bridge use.
- Do not implement provider interactivity, reply, or resume as part of MCP tasks.
- Do not make task status notifications required for correctness.
- Do not add a separate remote HTTP transport in this change.

## Decisions

### Decision: Research and gate before implementation

The first implementation task is a protocol compatibility memo that identifies the negotiated MCP version, capability shape, supported methods, and host behavior. Code changes for task support should not proceed until that memo is reflected in the design/specs.

Rationale: MCP tasks are explicitly experimental and have migration signals. Implementing the wrong wire contract would create churn and misleading compatibility claims.

Alternatives considered:

- Implement 2025-11-25 tasks immediately. Rejected because host support and future extension compatibility are unclear.
- Ignore MCP tasks entirely. Rejected because Agent Bridge's domain maps closely to durable protocol tasks and should be ready when hosts support them.

### Decision: Keep Agent Bridge task IDs and lifecycle as source of truth

MCP task responses should be derived from existing `TaskRecord` state. The MCP layer may present mapped statuses, TTL, poll interval, status messages, and cancellation, but it should not create a second task registry.

Rationale: The existing registry already persists provider tasks, logs, worktree state, diagnostics, and review packets. A second registry would drift.

Alternatives considered:

- Store separate MCP task records. Rejected because it duplicates identity, status, and cleanup state.

### Decision: Treat protocol cancellation as task stop, not cleanup

If a negotiated MCP task cancellation surface is implemented, it should map to stopping execution and finalizing cancellation semantics. It must not remove task state or managed worktrees automatically.

Rationale: Agent Bridge relies on post-run inspection before cleanup. MCP cancellation explicitly does not define deletion behavior, so retaining inspectability is safer.

Alternatives considered:

- Map cancellation to `task_remove`. Rejected because it can destroy inspectable worktree state.

### Decision: Notifications and progress are optional enhancements

Task status notifications and progress notifications should be implemented only when the active protocol/client capability supports them. Polling must remain sufficient for correctness.

Rationale: MCP task status notifications are optional; clients must not depend on them. Agent Bridge already has polling tools.

Alternatives considered:

- Require clients to consume notifications. Rejected because it would reduce host compatibility.

## Risks / Trade-offs

- Protocol churn could invalidate implementation -> Gate code behind a compatibility memo and keep `task_*` stable.
- Host support may be inconsistent -> Add compatibility fixtures for clients with and without task capabilities.
- Status mapping may lose Agent Bridge nuance -> Preserve detailed Agent Bridge status in existing tools and expose conservative MCP status messages.
- Cancellation semantics could imply cleanup -> Document and test that cancellation does not remove inspectable artifacts.

## Migration Plan

1. Write and validate a compatibility memo against current MCP task docs and host behavior.
2. Add protocol model tests for negotiated capabilities without changing task behavior.
3. Implement a minimal read/cancel/status mapping only if the memo identifies a stable supported surface.
4. Keep existing `task_*` tools documented as the primary lifecycle until protocol task support is proven in real hosts.
5. Roll back by disabling task capability advertisement; existing Agent Bridge lifecycle tools remain unchanged.

## Compatibility Memo Decisions

The compatibility memo must decide these before any task capability is advertised:

- Which task wire surface Agent Bridge targets: 2025-11-25 experimental tasks, a newer task extension, or a version-dispatched shim.
- Whether current Codex, Cursor, and Claude MCP clients can advertise and use protocol task capabilities over stdio.
- Whether Agent Bridge exposes protocol task listing when stdio has no auth context binding beyond the current process/session.
