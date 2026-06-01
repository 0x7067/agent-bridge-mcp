## Why

The Rust MCP server passes direct stdio fixtures, but live Codex MCP tool calls now fail before reaching provider logic because the host includes MCP-reserved `_meta` data in `tools/call` params. We also need a documented, testable workflow for using delegated provider tasks safely in real work instead of treating the bridge as only a protocol test target.

## What Changes

- Accept MCP-reserved metadata on the `tools/call` envelope while continuing to reject unknown public tool arguments inside `arguments`.
- Add real-host compatibility coverage that exercises Codex-style tool calls, including `_meta`, through the production stdio binary.
- Replace the single-root `AGENT_BRIDGE_ALLOWED_ROOT` configuration with a simpler `AGENT_BRIDGE_WORKSPACES` path-list. This is intentionally breaking; no backwards compatibility is preserved for the old variable.
- Define an operator workflow for provider readiness, task previewing, spawning, waiting, log inspection, final result inspection, and task cleanup.
- Add a local live-smoke harness path that can be run intentionally against installed provider CLIs without making live Claude/Cursor/Kimi/Codex execution mandatory in CI.
- Document provider selection guidance and isolation expectations for everyday use.

## Capabilities

### New Capabilities

- `mcp-host-compatibility`: MCP protocol compatibility requirements for real client hosts, including reserved `_meta` handling and production-binary compatibility fixtures.
- `delegation-workflow-harness`: Operational requirements for using provider task delegation in real workflows, including readiness checks, live smoke probes, task lifecycle discipline, and cleanup.

### Modified Capabilities

- None.

## Impact

- Affected code: `crates/agent-bridge-mcp/src/tools.rs`, `crates/agent-bridge-mcp/src/server.rs` if envelope parsing needs adjustment, and stdio compatibility tests.
- Affected docs: `README.md` and any local harness/runbook documentation added for live MCP usage.
- Affected runtime behavior: public tool argument schemas remain strict; MCP protocol metadata is tolerated at the request envelope; workspace confinement uses `AGENT_BRIDGE_WORKSPACES` instead of `AGENT_BRIDGE_ALLOWED_ROOT`.
- Dependencies: no new third-party dependency is expected.
