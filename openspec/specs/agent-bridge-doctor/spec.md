# agent-bridge-doctor Specification

## Purpose
Define the `doctor` diagnostics surface that helps operators verify Agent Bridge MCP configuration, workspace policy, state storage, provider readiness, and Claude host-runner setup without spawning normal provider tasks.
## Requirements
### Requirement: Doctor reports bridge setup status
The system SHALL expose a `doctor` MCP tool that returns a structured diagnostic report for the current Agent Bridge MCP process.

#### Scenario: Doctor is listed as a tool
- **WHEN** a client sends `tools/list`
- **THEN** the response includes a `doctor` tool with an input schema that rejects unknown fields.

#### Scenario: Default doctor report
- **WHEN** a caller invokes `doctor` with no arguments
- **THEN** the response includes `summary`, `server`, `workspace`, `state`, `providers`, `claudeHostRunner`, and `recommendations` sections.

#### Scenario: Doctor status levels
- **WHEN** doctor detects no blocking configuration or provider problems
- **THEN** `summary.status` is `ok`.
- **WHEN** doctor detects non-blocking concerns
- **THEN** `summary.status` is `warning`.
- **WHEN** doctor detects blocking setup failures
- **THEN** `summary.status` is `error`.

### Requirement: Doctor validates workspace and state configuration
The system SHALL diagnose workspace and state-dir configuration without spawning provider tasks.

#### Scenario: Missing workspace configuration
- **WHEN** `AGENT_BRIDGE_WORKSPACES` is missing or empty
- **THEN** doctor reports a workspace error and recommends setting `AGENT_BRIDGE_WORKSPACES`.

#### Scenario: Cwd outside workspace policy
- **WHEN** a caller passes `cwd` and it canonicalizes outside configured workspaces
- **THEN** doctor reports the cwd policy failure without spawning a task.

#### Scenario: Invalid cwd path
- **WHEN** a caller passes `cwd` that cannot be canonicalized
- **THEN** doctor reports the cwd validation failure without panicking and without spawning a task.

#### Scenario: State directory is usable
- **WHEN** the resolved state directory exists or can be created and its registry state is readable
- **THEN** doctor reports state status as ok.

#### Scenario: State directory is not usable
- **WHEN** the resolved state directory or registry cannot be created, read, or parsed
- **THEN** doctor reports a state error with a recommendation to inspect `AGENT_BRIDGE_STATE_DIR`.

### Requirement: Doctor aggregates provider readiness
The system SHALL include provider availability checks in doctor output.

#### Scenario: Default provider checks avoid smoke probes
- **WHEN** a caller invokes `doctor` without `smoke`
- **THEN** doctor runs provider version checks and reports `startupVerified: false` unless existing readiness logic proves otherwise.

#### Scenario: Optional smoke probes
- **WHEN** a caller invokes `doctor` with `smoke: true`
- **THEN** doctor runs the same bounded provider smoke readiness behavior as `providers_check`.

#### Scenario: Provider filtering
- **WHEN** a caller passes a provider filter
- **THEN** doctor checks only the selected providers using the same validation and deduplication semantics as `providers_check`.

#### Scenario: Duplicate provider filter entries
- **WHEN** a caller passes duplicate provider names
- **THEN** doctor checks each selected provider only once.

#### Scenario: Missing provider binary
- **WHEN** a selected provider binary is unavailable
- **THEN** doctor reports a provider warning or error and recommends installing or configuring that provider binary.

### Requirement: Doctor diagnoses Claude host-runner setup
The system SHALL report Claude host-runner configuration state separately from direct Claude provider readiness.

#### Scenario: Host runner not configured
- **WHEN** `AGENT_BRIDGE_CLAUDE_HOST_SOCKET` is not set
- **THEN** doctor reports host-runner status as not configured and explains that Claude uses direct launch strategy.

#### Scenario: Host runner configured and reachable
- **WHEN** `AGENT_BRIDGE_CLAUDE_HOST_SOCKET` is set and a bounded ping succeeds
- **THEN** doctor reports host-runner status as ok with protocol and workspace policy metadata.

#### Scenario: Host runner unavailable
- **WHEN** `AGENT_BRIDGE_CLAUDE_HOST_SOCKET` is set but the socket cannot be reached
- **THEN** doctor reports host-runner status as error within a bounded timeout and recommends starting or restarting the host runner.

#### Scenario: Workspace policy mismatch
- **WHEN** host-runner ping reports a workspace policy mismatch
- **THEN** doctor reports the mismatch and recommends restarting the runner with matching `AGENT_BRIDGE_WORKSPACES`.

### Requirement: Doctor recommendations are actionable and bounded
The system SHALL return concise recommendations derived from detected issues.

#### Scenario: Recommendations are ordered
- **WHEN** doctor detects multiple issues
- **THEN** recommendations are ordered from likely setup blockers to optional follow-up checks.

#### Scenario: No verification claims
- **WHEN** doctor reports recommendations or ok status
- **THEN** it does not claim delegated task output, project tests, or provider model behavior are verified.

#### Scenario: Secrets are not exposed
- **WHEN** the MCP process environment contains token, API key, OAuth, auth, or password values
- **THEN** doctor reports only presence or redacted indicators and does not include the raw secret values.

#### Scenario: Default doctor does not spawn delegated work
- **WHEN** a caller invokes `doctor` with no arguments
- **THEN** doctor does not create task records, does not execute task prompts, and does not call provider task modes.

### Requirement: Doctor distinguishes setup health from launch readiness
The system SHALL report provider launch readiness separately from overall setup health.

#### Scenario: Version-only provider check is not launch-ready
- **WHEN** doctor checks a provider with version-only probing and the provider binary is available
- **THEN** doctor reports the provider as available.
- **AND** doctor reports startup verification and launch readiness as not verified.

#### Scenario: Setup status can be ok while launch readiness is stale
- **WHEN** workspace, state, and host-runner setup have no blockers and provider checks are version-only
- **THEN** `summary.status` can remain `ok`.
- **AND** the response includes a separate launch-readiness signal that selected providers are not startup-verified.

#### Scenario: Smoke recommendation for selected stale providers
- **WHEN** a caller selects providers and those providers are available but not startup-verified
- **THEN** doctor recommends running `providers_check` or `doctor` with `smoke: true` before first launch when startup readiness matters.

### Requirement: Doctor recommendations include actionable tool arguments
The system SHALL include enough structured recommendation metadata for clients to offer follow-up actions without guessing.

#### Scenario: Recommendation includes follow-up call
- **WHEN** doctor recommends a follow-up Agent Bridge tool call
- **THEN** the recommendation includes the target tool name and minimal arguments needed for that follow-up when those arguments are known.

#### Scenario: Recommendation avoids secret leakage
- **WHEN** doctor creates structured recommendations from environment or provider data
- **THEN** the recommendation does not include raw secret values.
