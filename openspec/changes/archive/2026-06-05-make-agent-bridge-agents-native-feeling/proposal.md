## Why

Agent Bridge can already spawn and inspect provider tasks, but those tasks feel like MCP plumbing instead of first-class client agents. Native subagents expose a compact agent lifecycle and visible UI affordances; Agent Bridge needs a client-oriented contract so external providers can be presented with comparable clarity, status, and controls.

## What Changes

- Add a client-facing Agent Bridge presentation contract over existing `task_*` lifecycle tools.
- Expose compact task summaries under a nested `presentation` object suitable for native UI lists, status chips, and detail panes.
- Add `task_list` filter/search ergonomics for active and recent tasks so clients do not need to render the full historical task registry by default.
- Define standard client actions for wait, inspect, stop, cleanup, and unavailable actions such as reply/resume.
- Align runtime-discoverable provider and launch-profile metadata with the source/docs expectations needed by a native UI.
- Preserve the existing task lifecycle tools as the lower-level execution surface; this change is additive and not a replacement for task result verification.

## Capabilities

### New Capabilities

- `agent-bridge-agent-presentation`: Covers client-facing `presentation` summaries, lifecycle affordances, action availability, and list ergonomics for presenting Agent Bridge tasks as native-feeling agents while keeping `task_*` as the public API noun.

### Modified Capabilities

- `provider-adapter-contract`: Runtime provider capabilities must expose enough stable metadata for native clients to render supported modes, launch profiles, and action availability accurately.
- `delegation-workflow-harness`: Delegation workflows must describe the native-client path in addition to the raw MCP lifecycle path.

## Impact

- Affected code: MCP tool definitions and task lifecycle response shaping in `crates/agent-bridge-mcp/src/tools.rs`, `crates/agent-bridge-mcp/src/server.rs`, `crates/agent-bridge-mcp/src/task.rs`, provider capability reporting in `crates/agent-bridge-mcp/src/provider.rs`, and guidance in `crates/agent-bridge-mcp/src/guidance.rs`.
- Affected APIs: additive MCP fields and arguments for compact `presentation` summaries, `task_list` filters, action availability, and runtime capability metadata.
- Affected docs/specs: README examples, MCP prompts/resources, provider adapter contract, and delegation workflow guidance.
- Dependencies: no new third-party dependency is expected.
