## Context

Agent Bridge has a mature custom lifecycle exposed as `agent_spawn`,
`agent_list`, `agent_observe`, `agent_result`, `agent_stop`, and
`agent_remove`. Current MCP task documentation is split between the
experimental 2025-11-25 task surface and the newer
`io.modelcontextprotocol/tasks` extension surface.

The research found an important compatibility constraint: the 2025-11-25 experimental task surface is not guaranteed to be stable, and later task-extension work signals wire-level migration differences. Agent Bridge should not replace its stable lifecycle with an experimental protocol surface until host support and negotiated semantics are proven.

## Goals / Non-Goals

**Goals:**

- Produce a compatibility decision for MCP task support.
- Keep protocol-level MCP task support unadvertised and unimplemented until host
  and SDK compatibility is proven.
- Preserve existing `agent_*` tools as the stable lifecycle API.
- Cover clients that send no task metadata, current extension metadata, legacy
  metadata, unknown metadata, or conflicting metadata.
- Keep task result verification and managed-worktree cleanup boundaries intact.

**Non-Goals:**

- Do not remove or rename existing Agent Bridge lifecycle tools.
- Do not require MCP task support for normal Agent Bridge use.
- Do not add protocol task models, methods, or capability advertisement in this
  change.
- Do not implement provider interactivity, reply, or resume as part of MCP tasks.
- Do not make task status notifications required for correctness.
- Do not add a separate remote HTTP transport in this change.

## Decisions

### Decision: Research and gate before implementation

The first task is a protocol compatibility memo that identifies the negotiated
MCP version, capability shape, supported methods, and host behavior. Protocol
task implementation should not proceed until a target host and SDK support the
selected surface.

Rationale: MCP tasks are explicitly experimental and have migration signals. Implementing the wrong wire contract would create churn and misleading compatibility claims.

Alternatives considered:

- Implement 2025-11-25 tasks immediately. Rejected because host support and future extension compatibility are unclear.
- Ignore MCP tasks entirely. Rejected because Agent Bridge's domain maps closely to durable protocol tasks and should be ready when hosts support them.

### Decision: Read task metadata passively

Agent Bridge should inspect current, legacy, unknown, and conflicting
task-related client metadata for diagnostics, but it must not advertise tasks or
turn that metadata into public tool arguments.

Rationale: Passive diagnostics help future implementation work without creating
wire compatibility claims or leaking raw client metadata.

### Decision: Notifications and progress are optional enhancements

Task status notifications and progress notifications should be implemented only when the active protocol/client capability supports them. Polling must remain sufficient for correctness.

Rationale: MCP task status notifications are optional; clients must not depend on them. Agent Bridge already has polling tools.

Alternatives considered:

- Require clients to consume notifications. Rejected because it would reduce host compatibility.

## Risks / Trade-offs

- Protocol churn could invalidate implementation -> Keep protocol task methods unavailable and keep `agent_*` stable.
- Host support may be inconsistent -> Add readiness diagnostics for current, legacy, unknown, and absent task metadata.
- Status mapping may lose Agent Bridge nuance -> Do not add status mapping until a target extension surface is proven.
- Cancellation semantics could imply cleanup -> Do not add protocol cancellation until the negotiated surface is proven.

## Migration Plan

1. Write and validate a compatibility memo against current MCP task docs and host behavior.
2. Keep `initialize` free of task capabilities.
3. Return method-not-found for `tasks/*` protocol methods.
4. Surface task-extension readiness through `doctor` without leaking raw metadata.
5. Keep existing `agent_*` tools documented as the primary lifecycle until protocol task support is proven in real hosts.

## Compatibility Memo Decisions

See `compatibility-memo.md` for the current compatibility decision. The current
decision is blocked for protocol-level implementation: Agent Bridge must not
advertise task capabilities or implement `tasks/*` methods in this change.
Existing Agent Bridge `agent_*` tools remain the stable task lifecycle. A future
implementation must target a negotiated `io.modelcontextprotocol/tasks`
extension surface and add host fixtures for task-capable and non-task clients.

The compatibility memo must decide these before any task capability is advertised:

- Which task wire surface Agent Bridge targets: 2025-11-25 experimental tasks, a newer task extension, or a version-dispatched shim.
- Whether current Codex, Cursor, and Claude MCP clients can advertise and use protocol task capabilities over stdio.
- Whether Agent Bridge exposes protocol task listing when stdio has no auth context binding beyond the current process/session.
