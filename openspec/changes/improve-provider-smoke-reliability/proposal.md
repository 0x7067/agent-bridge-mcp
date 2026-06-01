## Why

Live `providers_check(smoke: true)` currently reports all providers as unavailable with a 10s timeout, even though normal delegated tasks using the same smoke phrase succeed when allowed to run longer. Provider readiness needs to reflect real task viability without exceeding Codex's MCP tool-call timeout or hiding provider-specific startup failures.

## What Changes

- Redesign provider smoke checks around measured provider task behavior instead of one fixed timeout for every provider.
- Add provider selection and additive aggregate runtime control so operators can smoke one provider or all providers without timing out the MCP host call.
- Add provider-specific startup budgets and readiness classifications that distinguish version availability, smoke timeout, auth/config failure, and successful task-path readiness.
- Make smoke prompts and command construction intentionally minimal, avoiding full workflow/instruction loading when a cheaper provider-native smoke path exists.
- Preserve deterministic fake-provider tests while adding live-smoke guidance and fixtures that model slow-but-successful providers.

## Capabilities

### New Capabilities

- `provider-readiness-contract`: Defines reliable provider readiness checks, provider-specific smoke budgets, provider filtering, bounded aggregate runtime, and diagnostic behavior for live provider startup probes.

### Modified Capabilities

- None.

## Impact

- Affected code: `crates/agent-bridge-mcp/src/server.rs`, `crates/agent-bridge-mcp/src/provider.rs`, provider command construction, diagnostics, and stdio tests.
- Affected API: `providers_check` gains optional additive inputs for provider filtering and aggregate smoke budget behavior. Existing `smoke` and `timeoutMs` remain supported; `timeoutMs` keeps its per-provider timeout role.
- Affected docs: README provider workflow and live smoke guidance.
- Dependencies: no new third-party dependency is expected.
