## ADDED Requirements

### Requirement: Claude adapter resolves only owned interactive execution
The system SHALL resolve Claude provider task execution and smoke probes to the owned interactive runner path, not to external print-mode compatibility binaries.

#### Scenario: Claude task command resolution
- **WHEN** the provider adapter builds a Claude task launch descriptor
- **THEN** it selects the owned interactive Claude runner launch strategy.
- **AND** it does not select upstream `claude-p` or native `claude -p`.

#### Scenario: Claude smoke command resolution
- **WHEN** the provider adapter builds a Claude smoke probe
- **THEN** it exercises the same owned interactive runner path as normal Claude task execution.

#### Scenario: Claude version-only check
- **WHEN** the provider adapter performs a Claude version-only readiness check
- **THEN** it may check that the official `claude` binary is discoverable.
- **AND** it does not mark Claude as launchable without an owned-runner smoke success.

#### Scenario: Claude binary override
- **WHEN** the provider adapter resolves the official interactive Claude binary
- **THEN** it uses `CLAUDE_BIN` as the official interactive `claude` executable override if configured and otherwise searches `PATH` for `claude`.
- **AND** it does not use `CLAUDE_P_BIN` for task execution, smoke checks, or version checks.

#### Scenario: Legacy claude-p environment
- **WHEN** `CLAUDE_P_BIN` is present in the environment
- **THEN** provider diagnostics identify it as ignored legacy configuration for the owned Claude provider.

### Requirement: Claude adapter reports removed fallback policy
The system SHALL make removed Claude fallback behavior explicit in capability metadata and diagnostics.

#### Scenario: Listing provider capabilities
- **WHEN** a caller invokes `providers_list`
- **THEN** the Claude provider metadata reports the owned interactive runner as the supported launch path.
- **AND** native print-mode fallback and upstream `claude-p` fallback are not advertised as supported.

#### Scenario: Claude runner is unavailable
- **WHEN** owned interactive Claude execution cannot be started
- **THEN** diagnostics explain the owned-runner failure.
- **AND** diagnostics do not recommend switching to native `claude -p`.

### Requirement: Claude adapter accepts owned-runner smoke output
The system SHALL treat owned-runner transcript completion as the Claude smoke success contract instead of print-mode JSON stdout.

#### Scenario: Owned-runner smoke succeeds
- **WHEN** a Claude smoke probe completes through the owned interactive runner and the final transcript result contains `AGENT_BRIDGE_PROVIDER_SMOKE_OK`
- **THEN** provider readiness marks Claude startup verified and launchable.

#### Scenario: Owned-runner smoke prompt
- **WHEN** the Claude provider builds a smoke probe
- **THEN** it uses the non-mutating prompt `Reply with exactly: AGENT_BRIDGE_PROVIDER_SMOKE_OK`.
- **AND** it accepts success only through owned-runner Stop/transcript completion.

#### Scenario: Print-mode JSON appears
- **WHEN** Claude output contains legacy print-mode JSON without owned-runner transcript completion
- **THEN** the Claude smoke probe does not treat that JSON alone as startup verification.
