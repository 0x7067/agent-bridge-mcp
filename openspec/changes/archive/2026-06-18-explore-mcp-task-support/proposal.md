## Why

Agent Bridge already has a custom `agent_*` lifecycle for long-running provider work, while the current MCP specification includes experimental task primitives for durable request status, polling, cancellation, progress, and deferred result retrieval. Agent Bridge should evaluate and, where safe, expose protocol-aligned task support without destabilizing existing clients.

## What Changes

- Research the current MCP task specifications and compatibility constraints, including experimental 2025-11-25 tasks and newer extension migration signals.
- Add a compatibility decision for MCP task concepts such as task status,
  cancellation, progress, and task-extension negotiation.
- Keep protocol-level MCP task support unadvertised and unimplemented until a
  target host and SDK ship a compatible negotiated surface.
- Preserve the existing `agent_*` tools as the stable Agent Bridge lifecycle
  surface.
- Do not require clients to adopt MCP tasks to use Agent Bridge.

## Capabilities

### New Capabilities

- `mcp-task-support`: Covers protocol-level task compatibility gating for Agent Bridge, including capability negotiation constraints, unsupported protocol methods, progress/status notification behavior, and migration constraints.

### Modified Capabilities

- `rust-single-binary-mcp`: The Rust MCP public protocol surface must not advertise or implement task-related methods before a supported negotiated surface exists.
- `delegation-workflow-harness`: Delegation workflows must describe when to use native Agent Bridge `agent_*` tools versus protocol-level MCP task support.
- `mcp-host-compatibility`: Host compatibility requirements must cover clients that advertise current, legacy, unknown, or no MCP task metadata.

## Impact

- Affected code: MCP request dispatch in `crates/agent-bridge-mcp/src/server.rs`, task-extension readiness diagnostics, and stdio compatibility tests.
- Affected APIs: no new MCP task API is exposed in this change. Existing Agent Bridge `agent_*` tools remain supported.
- Affected docs/specs: README, host compatibility guidance, delegation workflow guidance, and Rust binary protocol requirements.
- Dependencies: no new third-party dependency is expected for the research/design slice; implementation should prefer existing serde/JSON-RPC infrastructure.
