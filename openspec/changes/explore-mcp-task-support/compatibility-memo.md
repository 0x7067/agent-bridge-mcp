# MCP Task Compatibility Memo

## Decision

Agent Bridge should not advertise or implement protocol-level MCP task support in this change. Keep the existing Agent Bridge `task_*` tools as the stable task lifecycle and treat protocol tasks as blocked until a concrete target host negotiates the current extension surface over stdio.

## Source Review

- The 2025-11-25 MCP Tasks specification says tasks are experimental and may evolve. It uses request `task` parameters plus `tasks/get`, `tasks/list`, `tasks/result`, and `tasks/cancel`.
- SEP-2663 is now a Final Extensions Track document for `io.modelcontextprotocol/tasks`. It uses extension negotiation, `CreateTaskResult` with `resultType: "task"`, `tasks/get`, `tasks/update`, and `tasks/cancel`.
- The extension remains wire-incompatible with the 2025-11-25 experimental task surface. The extension removes `tasks/result`, removes `tasks/list`, ignores the legacy request `task` parameter under the extension surface, and forbids continuing to advertise legacy task capabilities under protocol versions that include the extension.
- The extension overview says host support varies and task support requires explicit opt-in from both client and server. Current official client-matrix documentation does not list Tasks support as an extension row, so it does not provide first-party evidence that Codex, Cursor, Claude, or other target hosts currently negotiate `io.modelcontextprotocol/tasks`.

References:

- https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/tasks
- https://modelcontextprotocol.io/extensions/tasks/overview
- https://modelcontextprotocol.io/seps/2663-tasks-extension
- https://modelcontextprotocol.io/extensions/client-matrix

## Research Refresh - 2026-06-05

The official docs still support the existing blocked implementation decision.
The important update is that the extension itself is no longer merely a
migration signal: SEP-2663 is Final. That makes the target surface clearer, but
does not make it safe for Agent Bridge to advertise protocol tasks yet.

Implementation remains blocked because:

- Agent Bridge currently answers `initialize` with protocol version `2024-11-05`
  and capabilities `{ "tools": {}, "prompts": {}, "resources": {} }`, with no
  `tasks` or extension advertisement.
- The supported extension shape depends on explicit `io.modelcontextprotocol/tasks`
  negotiation and per-request client capability metadata before a server may
  return `CreateTaskResult`.
- The legacy `2025-11-25` task surface is not compatible with the extension
  surface planned for the future `2026-06-30` protocol line.
- The official client matrix does not currently establish task-extension support
  for the target hosts Agent Bridge cares about.
- The follow-up readiness-probe change is the correct current strategy: observe
  extension-capable, legacy-only, unknown, and unsupported metadata passively
  while keeping protocol task methods unavailable.

## Research Refresh - 2026-06-18

The implementation decision remains blocked. Current primary docs still show two
different task surfaces:

- The 2025-11-25 `basic/utilities/tasks` page still labels tasks experimental.
  It advertises a legacy `tasks` capability with `tasks/list`, `tasks/cancel`,
  and task-augmented request categories.
- The current Tasks extension overview instead uses
  `io.modelcontextprotocol/tasks` extension negotiation, `CreateTaskResult`,
  and `tasks/get`, `tasks/update`, and `tasks/cancel`. It also says servers must
  verify per-request client extension support before returning a task result.
- The official extension support matrix does not list Tasks in its extension
  overview table and does not provide evidence that target hosts currently
  negotiate the Tasks extension.
- The TypeScript, Python, and Java SDK trackers for SEP-2663 are still open;
  Python and Java explicitly tie the work to the future 2026-07-28 spec release,
  while TypeScript tracks the same SEP in the 2026-07-28 implementation project.

So Agent Bridge should keep protocol-level task support unadvertised and
unimplemented for now. Existing `agent_*` lifecycle tools and completion
notifications remain the native collaboration path. Revisit this only when a
target host and SDK have shipped `io.modelcontextprotocol/tasks` negotiation over
stdio.

## Host Compatibility

Current target hosts should be treated as non-task clients unless they explicitly negotiate a task extension in their MCP capabilities. Agent Bridge currently initializes with protocol version `2024-11-05` and does not advertise task capabilities. Existing clients can continue using `task_spawn`, `task_list`, `task_status`, `task_wait`, `task_logs`, `task_transcript`, `task_result`, `task_stop`, and `task_remove` over stdio.

No host-specific protocol task capability should be inferred from ordinary stdio MCP support. A future implementation must add fixtures for both client shapes:

- client without task extension support: no task capabilities advertised; existing tools work unchanged.
- client with `io.modelcontextprotocol/tasks` support: only negotiated and implemented methods are advertised and exercised.

## Supported Surface For This Change

Supported now:

- No protocol-level task capability advertisement.
- No `tasks/*` method implementation.
- Documentation distinguishing Agent Bridge lifecycle tools from protocol-level MCP task support.
- Continue deriving task state, presentation, review packets, logs, transcripts, and cleanup semantics from the existing task registry.

Blocked until future compatibility work:

- Typed MCP task protocol models.
- Task capability advertisement.
- Protocol task polling, update, result, cancellation, listing, and notifications.

## Task Listing Decision

Do not expose protocol-level task listing. The 2025-11-25 task list semantics require caller-scoped retrievability, and the newer extension removes task listing to avoid cross-caller leakage. Agent Bridge stdio sessions do not yet have a protocol-level auth or caller-binding model for protocol tasks, so listing should remain available only through the explicit Agent Bridge `task_list` tool.

## Cancellation Decision

No protocol-level cancellation is implemented in this change. If implemented later, cancellation must route to `task_stop` semantics and must not remove logs, results, registry records, or managed worktrees automatically.

## Notification Decision

No protocol task status/progress notifications are in scope. Polling through existing `task_wait`, `task_status`, `task_logs`, and `task_transcript` remains sufficient for correctness.
