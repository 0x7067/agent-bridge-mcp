## MODIFIED Requirements

### Requirement: Provider readiness checks support provider filtering
The system SHALL allow callers to restrict `providers_check` to one or more selected providers, including `antigravity`.

#### Scenario: Single provider smoke
- **WHEN** a caller invokes `providers_check` with `smoke: true` and a provider filter containing `antigravity`
- **THEN** the response includes a readiness result for `antigravity`, omits unselected providers from the response array, and does not spend time smoking unselected providers.

#### Scenario: Invalid provider filter
- **WHEN** a caller invokes `providers_check` with an unknown provider in the provider filter
- **THEN** the tool returns a validation error listing the supported provider names, including `antigravity`.

### Requirement: Provider smoke probes use provider-specific budgets
The system SHALL support provider-specific smoke budgets while retaining a safe default budget for providers without explicit overrides.

#### Scenario: Default provider budgets
- **WHEN** a caller invokes `providers_check` with `smoke: true` and no provider-specific timeout overrides
- **THEN** the bridge uses bounded default smoke budgets for all supported providers, including Antigravity.

#### Scenario: Antigravity authentication required
- **WHEN** Antigravity answers `--version` but `agy --print` requires authentication or otherwise fails to return the smoke token
- **THEN** the provider response preserves version-probe availability.
- **AND** the provider response reports `startupVerified: false`, `launchable: false`, and bounded diagnostics explaining the failed smoke phase.

### Requirement: Smoke probes exercise task path with minimal prompt
The system SHALL use a minimal non-mutating smoke prompt or provider-native smoke mode that still exercises the provider binary, environment policy, cwd handling, and output parsing path.

#### Scenario: Antigravity smoke uses print mode
- **WHEN** a caller invokes `providers_check` with `smoke: true` for `antigravity`
- **THEN** the smoke command uses Antigravity print mode with the minimal smoke prompt and a bounded print timeout.
