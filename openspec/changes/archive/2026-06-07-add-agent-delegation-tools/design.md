# Design

## API Shape

The public MCP tool surface uses one canonical `agent_*` namespace. `agent_spawn` starts a provider agent through the existing task manager spawn path and returns the existing lifecycle payload, including the persisted `taskId`. The task registry and `task_...` identifier format remain internal implementation details.

`agent_list` is the presentation-first listing tool. It accepts status, provider, mode, workspace, title, and limit filters and always uses bounded active/recent presentation mode. The response returns `agents` plus list metadata (`scope: "active_recent"`, `limit`) so clients do not need to know that provider agents are backed by task records.

Lifecycle inspection and cleanup also use the canonical namespace:

- `agent_status`
- `agent_wait`
- `agent_logs`
- `agent_transcript`
- `agent_result`
- `agent_stop`
- `agent_remove`

`agent_preview` replaces `task_preview` for launch inspection. The preview response may still mention internal task-manager details when useful, but the tool name presented to MCP clients remains agent-oriented.

## Removal Strategy

The previous `task_*` public MCP tools are removed from `tools/list` once the `agent_*` equivalents have protocol tests, stdio smoke coverage, and guidance updates. The server does not need to preserve a second public tool set because the target harnesses are being taught the canonical surface before this change is shipped. If compatibility aliases are ever needed later, they should be hidden or feature-gated rather than reintroduced as a normal advertised workflow.

## Guidance

Initialization instructions and guidance resources should direct new callers to:

1. run `doctor` when setup is uncertain,
2. use `agent_spawn` to launch provider agents,
3. use `agent_list` for active/recent native presentation,
4. use `agent_observe`, `agent_status`, `agent_logs`, `agent_transcript`, `agent_wait`, and `agent_result` for monitoring and final evidence,
5. run caller-owned verification before claiming delegated work is done.
