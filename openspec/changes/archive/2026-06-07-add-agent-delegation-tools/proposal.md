# Add agent delegation tools

## Why
Harnesses and MCP clients currently have to understand Agent Bridge through low-level `task_*` tools. The runtime already exposes native-feeling task presentation metadata, but the primary launch/listing verbs still read as generic background tasks rather than provider subagents.

Adding a single `agent_*` tool family gives clients a simpler agent-oriented entry point while preserving the existing internal task registry and verification boundaries.

## What Changes
- Add `agent_list` as the active/recent agent presentation list.
- Add `agent_spawn` as the preferred provider-agent launch tool.
- Add `agent_status`, `agent_wait`, `agent_logs`, `agent_transcript`, `agent_result`, `agent_stop`, and `agent_remove` as the public lifecycle tools.
- Remove the parallel public `task_*` tool family from `tools/list` after the canonical `agent_*` path is covered by protocol tests and local smoke coverage.
- Update prompts/resources so self-guided clients use one agent-oriented surface for launch, list, status, observation, logs, transcript, result inspection, stop, and cleanup.

## Non-Goals
- Do not implement protocol-level MCP Tasks or advertise `io.modelcontextprotocol/tasks`.
- Do not rename persisted task IDs or internal task records in this change.
- Do not keep duplicate public MCP tool families once the replacement path is confirmed working.

## Impact
- Affected code: Rust MCP tool definitions, tool dispatch, task listing/spawn wrappers, guidance prompts/resources, stdio protocol tests.
- Affected specs: `agent-bridge-agent-presentation`, `mcp-usage-guidance`, `rust-single-binary-mcp`.
