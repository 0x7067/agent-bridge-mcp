# Add agent delegation tools

## Why
Harnesses and MCP clients currently have to understand Agent Bridge through low-level `task_*` tools. The runtime already exposes native-feeling task presentation metadata, but the primary launch/listing verbs still read as generic background tasks rather than provider subagents.

Adding `agents_list` and `agent_spawn` gives clients a simpler agent-oriented entry point while preserving the existing lifecycle tools and verification boundaries.

## What Changes
- Add `agents_list` as the preferred active/recent agent presentation list.
- Add `agent_spawn` as the preferred provider-agent launch tool.
- Keep `task_spawn` available as a legacy alias until `agents_list` and `agent_spawn` are verified with protocol tests and local smoke coverage.
- Update prompts/resources so self-guided clients prefer the agent-oriented surface and use `task_*` lifecycle tools for status, waiting, logs, transcript, result inspection, stop, and cleanup.
- Document that `task_spawn` removal is a later compatibility step, not part of this change.

## Non-Goals
- Do not implement protocol-level MCP Tasks or advertise `io.modelcontextprotocol/tasks`.
- Do not rename task IDs or lifecycle tools in this change.
- Do not remove `task_spawn` until the replacement path is confirmed working.

## Impact
- Affected code: Rust MCP tool definitions, tool dispatch, task listing/spawn wrappers, guidance prompts/resources, stdio protocol tests.
- Affected specs: `agent-bridge-agent-presentation`, `mcp-usage-guidance`, `rust-single-binary-mcp`.
