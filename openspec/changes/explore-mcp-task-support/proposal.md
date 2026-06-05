## Why

Agent Bridge already has a custom `task_*` lifecycle for long-running provider work, while the current MCP specification includes experimental task primitives for durable request status, polling, cancellation, progress, and deferred result retrieval. Agent Bridge should evaluate and, where safe, expose protocol-aligned task support without destabilizing existing clients.

## What Changes

- Research the current MCP task specifications and compatibility constraints, including experimental 2025-11-25 tasks and newer extension migration signals.
- Add a compatibility design for mapping Agent Bridge task records to MCP task concepts such as `working`, `completed`, `failed`, `cancelled`, TTL, poll interval, status message, and cancellation.
- Implement a bounded compatibility slice only after the design chooses a negotiated protocol strategy.
- Preserve the existing `task_*` tools as the stable Agent Bridge lifecycle surface.
- Do not require clients to adopt MCP tasks to use Agent Bridge.

## Capabilities

### New Capabilities

- `mcp-task-support`: Covers protocol-level task compatibility for Agent Bridge, including capability negotiation, task status mapping, cancellation semantics, progress/status notification behavior, and migration constraints.

### Modified Capabilities

- `rust-single-binary-mcp`: The Rust MCP public protocol surface may need negotiated support for task-related methods, notifications, and compatibility fixtures.
- `delegation-workflow-harness`: Delegation workflows must describe when to use native Agent Bridge `task_*` tools versus protocol-level MCP task support.
- `mcp-host-compatibility`: Host compatibility requirements must cover clients that do and do not advertise MCP task support.

## Impact

- Affected code: MCP request dispatch in `crates/agent-bridge-mcp/src/server.rs`, task manager/status/cancellation behavior in `crates/agent-bridge-mcp/src/task.rs`, protocol models in `crates/agent-bridge-mcp/src/mcp.rs`, and stdio compatibility tests.
- Affected APIs: possible additive MCP task capabilities and methods such as task status/list/cancel/result surfaces, depending on negotiated design. Existing Agent Bridge `task_*` tools remain supported.
- Affected docs/specs: README, host compatibility guidance, delegation workflow guidance, and Rust binary protocol requirements.
- Dependencies: no new third-party dependency is expected for the research/design slice; implementation should prefer existing serde/JSON-RPC infrastructure.
