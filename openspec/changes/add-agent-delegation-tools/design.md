# Design

## API Shape

`agent_spawn` is an additive launch tool with the same input schema as `task_spawn`. It calls the same task manager spawn path and returns the same task lifecycle payload, including `taskId`. The existing lifecycle remains task-based: callers continue to use `task_status`, `task_wait`, `task_logs`, `task_transcript`, `task_result`, `task_stop`, and `task_remove`.

`agents_list` is an additive presentation-first listing tool. It accepts the current `task_list` presentation filters except `presentation` and `scope`; it always uses bounded active/recent presentation mode. The response returns `agents` plus the same list metadata (`scope: "active_recent"`, `limit`) so clients do not need to know that provider agents are backed by task records. Raw/full-history registry inspection remains available through `task_list`.

## Deprecation Strategy

`task_spawn` remains in `tools/list` for this change and is marked as legacy in the description and guidance. Removal requires a later change after `agent_spawn` and `agents_list` are verified against the target harnesses. This avoids breaking existing clients while allowing new clients to migrate immediately.

## Guidance

Initialization instructions and guidance resources should direct new callers to:

1. run `doctor` when setup is uncertain,
2. use `agent_spawn` to launch provider agents,
3. use `agents_list` for active/recent native presentation,
4. use task lifecycle tools for monitoring and final evidence,
5. run caller-owned verification before claiming delegated work is done.

Guidance should still mention `task_spawn` as a legacy compatibility surface until removal.
