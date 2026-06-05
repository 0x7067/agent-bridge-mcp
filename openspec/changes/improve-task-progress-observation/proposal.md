## Why

Agent Bridge providers can be alive and useful while producing no stdout until final JSON, especially Cursor. Current guidance and lifecycle metadata make that silence look like a stall, which causes callers to stop or fall back too early.

## What Changes

- Add a progress-observation surface that lets callers distinguish "silent but still within expected provider behavior" from "probably stalled".
- Add adaptive polling guidance and machine-readable wait recommendations based on provider, mode, profile, timeout, elapsed time, first-output state, and recent transcript activity.
- Expose provider progress characteristics so harnesses can choose sensible wait intervals and fallback thresholds without hardcoding Cursor-specific behavior.
- Add an optional long-poll/event read path that returns new lifecycle/transcript events since a cursor or after a bounded wait.
- Update guidance so callers do not stop Cursor or other final-output providers solely because logs/transcript show only `spawned`.
- Do not implement protocol-level MCP Tasks notifications or server-pushed JSON-RPC notifications in this change.

## Capabilities

### New Capabilities
- `task-progress-observation`: Progress state, adaptive polling recommendations, and bounded event-style observation for running provider tasks.

### Modified Capabilities
- `agent-bridge-agent-presentation`: Presentation and `nextActions` should surface progress-observation recommendations for running tasks.
- `mcp-usage-guidance`: Prompts/resources should teach callers how to poll, long-poll, and avoid premature stop/fallback decisions for silent providers.
- `provider-adapter-contract`: Provider metadata should describe expected output cadence and recommended observation budgets.

## Impact

- Affected API: additive task progress fields, additive provider metadata, and possibly a new tool such as `task_observe` or additive parameters on `task_transcript`.
- Affected code: task manager lifecycle records, transcript/log observation, presentation `nextActions`, provider capability metadata, guidance resources, stdio protocol tests.
- Affected systems: MCP harnesses that delegate to Cursor/Claude/Kimi/Codex and decide when to wait, inspect, stop, or fall back.
