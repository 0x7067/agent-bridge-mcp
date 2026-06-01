## Why

Agent Bridge has several reliable diagnostics surfaces, but operators still need to combine `providers_check`, `task_preview`, host-runner guidance, environment variables, and state-dir knowledge by hand when setup fails. A single doctor check should summarize configuration, readiness, and likely next actions without spawning delegated tasks by default.

## What Changes

- Add a new `doctor` MCP tool that returns an operator-focused diagnostic report for the current MCP process.
- Include server/runtime metadata, workspace policy status, state-dir status, provider version readiness, Claude host-runner configuration status, and actionable recommendations.
- Keep live provider smoke probes opt-in through an explicit `smoke` argument; default doctor checks stay deterministic and cheap.
- Extend MCP guidance/docs to point operators at `doctor` before deeper provider or host-runner troubleshooting.

## Capabilities

### New Capabilities
- `agent-bridge-doctor`: Operator diagnostics for bridge configuration, provider availability, workspace policy, state directory, host-runner setup, and recommended next actions.

### Modified Capabilities
- `mcp-usage-guidance`: Add discoverable guidance that recommends `doctor` as the first troubleshooting step for setup and provider issues.
- `rust-single-binary-mcp`: Document the additive `doctor` tool in the public MCP tool surface.

## Impact

- Affected code: tool definitions, server tool dispatch, provider/version check reuse, workspace/state-dir diagnostics, optional host-runner ping diagnostics, protocol tests, stdio tests.
- Affected API: adds a new MCP tool named `doctor`; no existing tool input or response shape changes.
- Affected docs: README troubleshooting/workflow sections and MCP guidance resources.
- Dependencies: no new third-party dependency is expected.
