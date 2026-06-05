## Why

Agent Bridge should be able to observe whether MCP clients can negotiate the current Tasks extension before it implements protocol-level task support. The previous compatibility memo deliberately blocked task implementation; readiness probes give us evidence without advertising unsupported task capabilities.

## What Changes

- Add a non-advertising readiness probe path that records task-extension capability shapes seen from MCP clients.
- Add deterministic compatibility fixtures for clients with no task extension metadata, legacy 2025-11-25-style metadata, and current `io.modelcontextprotocol/tasks` extension metadata.
- Expose diagnostic output that says whether protocol-level task support is unavailable, unsupported, legacy-only, or extension-capable.
- Surface the diagnostic output as an additive `doctor.taskExtensionReadiness` section.
- Keep Agent Bridge `task_*` tools as the only task execution lifecycle in this change.
- Do not add `tasks/*` methods, `CreateTaskResult`, task capability advertisement, task cancellation, or protocol task listing.

## Capabilities

### New Capabilities

- `task-extension-readiness`: Covers non-advertising detection and diagnostics for MCP task-extension capability shapes.

### Modified Capabilities

- `mcp-host-compatibility`: Host compatibility requirements must include deterministic client capability fixtures for task-extension readiness probing without enabling protocol tasks.
- `mcp-usage-guidance`: Guidance must explain that readiness probes are diagnostic evidence only and do not make protocol task support available.
- `agent-bridge-doctor`: Doctor output must include the additive task-extension readiness diagnostic without changing setup status aggregation or advertising protocol task support.

## Impact

- Affected code: MCP request metadata parsing in `crates/agent-bridge-mcp/src/server.rs`, diagnostic/result shaping for readiness probe output, and stdio compatibility tests.
- Affected APIs: additive diagnostic metadata or a narrow diagnostic tool/resource for task-extension readiness; no task execution API changes.
- Affected docs/specs: host compatibility guidance, MCP usage guidance, and task readiness probe contract.
- Dependencies: no new third-party dependency expected.
