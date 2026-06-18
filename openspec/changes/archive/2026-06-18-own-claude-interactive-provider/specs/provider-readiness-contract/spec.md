## MODIFIED Requirements

### Requirement: Provider smoke probes use provider-specific budgets
The system SHALL support provider-specific smoke budgets while retaining a safe default budget for providers without explicit overrides.

#### Scenario: Default provider budgets
- **WHEN** a caller invokes `providers_check` with `smoke: true` and no provider-specific timeout overrides
- **THEN** the bridge uses default smoke budgets of 20000ms for `codex`, 60000ms for owned interactive `claude`, 45000ms for `kimi`, and 60000ms for `cursor`.

#### Scenario: Invalid provider-specific budget
- **WHEN** a caller supplies a provider-specific budget that is zero, negative, non-integer, or greater than 90000
- **THEN** the tool returns a validation error identifying the invalid provider budget.

#### Scenario: Invalid provider-specific budget key
- **WHEN** a caller supplies a `providerTimeoutMs` key that is not a supported provider name
- **THEN** the tool returns a validation error listing the supported provider names.

#### Scenario: Slow provider succeeds within its budget
- **WHEN** a provider smoke probe takes longer than the default short timeout but completes within its configured provider budget
- **THEN** the provider response reports `startupVerified: true`.

#### Scenario: Provider exceeds its budget
- **WHEN** a provider smoke probe exceeds its provider-specific budget
- **THEN** the provider response reports `startupVerified: false` with `failureCategory: "provider_timeout"`.

### Requirement: Smoke probes exercise task path with bounded prompts
The system SHALL use a minimal non-mutating smoke prompt or provider-native smoke mode that still exercises the provider binary, environment policy, cwd handling, and output parsing path.

#### Scenario: Minimal prompt avoids normal workflow overhead
- **WHEN** a provider has a dedicated smoke prompt or smoke command builder
- **THEN** the smoke probe avoids unnecessary full task instructions, context loading, or workflow text while still requiring parseable provider output.

#### Scenario: Claude owned-runner smoke token
- **WHEN** the Claude provider runs a smoke probe
- **THEN** it uses the prompt `Reply with exactly: AGENT_BRIDGE_PROVIDER_SMOKE_OK`.
- **AND** the smoke probe succeeds only when owned-runner Stop/transcript completion produces `AGENT_BRIDGE_PROVIDER_SMOKE_OK`.

#### Scenario: Smoke remains non-mutating
- **WHEN** a smoke probe runs for any provider
- **THEN** the prompt and command mode do not intentionally edit files or require write-capable permissions.

#### Scenario: Minimal smoke fallback is reported
- **WHEN** a provider cannot safely use the minimal smoke renderer and falls back to the standard task prompt
- **THEN** the provider response reports the smoke prompt strategy so slow startup remains explainable.
