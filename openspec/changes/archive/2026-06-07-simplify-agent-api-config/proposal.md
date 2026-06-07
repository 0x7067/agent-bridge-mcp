## Why

The public lifecycle tools now use the `agent_*` namespace, but their request and response payloads still force callers to think in `taskId` terms. Configuration also has an inconsistent default state directory between runtime task storage and `doctor`, which makes a "minimal config" setup ambiguous.

## What Changes

- **BREAKING** Replace public `taskId` request and response fields with `agentId` across the advertised `agent_*` lifecycle surface.
- **BREAKING** Reject `taskId` as an unknown public argument on `agent_*` lifecycle tools.
- Keep internal task records and on-disk registry implementation details private; public MCP responses and next-action metadata must not require callers to understand the internal task model.
- Make the default state directory consistent across runtime task storage and `doctor`: `~/.agent-bridge-mcp/state`.
- Update self-guidance, README, specs, and protocol tests so the discoverable API and configuration story name only the simplified fields and default.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `rust-single-binary-mcp`: public lifecycle schemas expose and accept only canonical `agentId`, and runtime/doctor state defaults match.
- `agent-bridge-self-guidance`: self-guided surfaces and next actions use canonical `agent_*`/`agentId` terminology.
- `mcp-usage-guidance`: prompts/resources document the simpler agent-oriented lifecycle and minimal configuration.
- `delegation-workflow-harness`: operator workflow docs describe the canonical lifecycle identifier and default state directory.
- `agent-bridge-agent-presentation`: presentation summaries expose agent identifiers and agent lifecycle controls.
- `delegated-review-packet`: review packets are returned by `agent_result` and use agent identifiers/actions.
- `provider-adapter-contract`: provider adapter requirements preserve the canonical `agent_*` public API rather than old `task_*` names.
- `task-launch-profiles`: launch-profile requirements use `agent_preview`, `agent_spawn`, and `agent_result`.
- `codex-provider-reliability`: Codex denial requirements use canonical `agent_*` lifecycle evidence.
- `mcp-host-compatibility`: strict argument validation examples use canonical `agent_preview`.

## Impact

- Affected code: Rust MCP tool definitions, lifecycle dispatch argument parsing, task response rendering, next-action metadata, doctor state diagnostics.
- Affected tests: protocol schema tests and stdio lifecycle/config tests.
- Affected docs/specs: README, guidance prompts/resources, and OpenSpec deltas for the modified capabilities.
- Compatibility: this is an intentional public API break. Existing callers must rename `taskId` arguments and response reads to `agentId`; existing registries serialized with public `taskId` fields are not migrated.
