# MCP Task Compatibility Memo

## Decision

Agent Bridge should not advertise or implement protocol-level MCP task support in this change. Keep the existing Agent Bridge `task_*` tools as the stable task lifecycle and treat protocol tasks as blocked until a concrete target host negotiates the current extension surface over stdio.

## Source Review

- The 2025-11-25 MCP Tasks specification says tasks are experimental and may evolve. It uses request `task` parameters plus `tasks/get`, `tasks/list`, `tasks/result`, and `tasks/cancel`.
- The newer MCP Tasks extension uses extension negotiation through `io.modelcontextprotocol/tasks`, `CreateTaskResult` with `resultType: "task"`, `tasks/get`, `tasks/update`, and `tasks/cancel`. It also states that the legacy 2025-11-25 capability declarations are not part of the extension and must not be advertised under protocol versions that include the extension.
- The extension overview says host support varies and task support requires explicit opt-in from both client and server.

References:

- https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/tasks
- https://modelcontextprotocol.io/extensions/tasks/overview
- https://modelcontextprotocol.io/seps/2663-tasks-extension

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
