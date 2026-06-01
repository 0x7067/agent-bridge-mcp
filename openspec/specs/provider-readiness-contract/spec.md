# provider-readiness-contract Specification

## Purpose
Define provider availability and startup-readiness checks, including provider filtering, bounded aggregate smoke probes, provider-specific budgets, timing fields, and actionable diagnostics.
## Requirements
### Requirement: Provider readiness checks support provider filtering
The system SHALL allow callers to restrict `providers_check` to one or more selected providers.

#### Scenario: Single provider smoke
- **WHEN** a caller invokes `providers_check` with `smoke: true` and a provider filter containing `cursor`
- **THEN** the response includes a readiness result for `cursor`, omits unselected providers from the response array, and does not spend time smoking unselected providers

#### Scenario: Single provider version check
- **WHEN** a caller invokes `providers_check` with `smoke: false` and a provider filter containing `cursor`
- **THEN** the response includes a version result for `cursor` and omits unselected providers from the response array

#### Scenario: Invalid provider filter
- **WHEN** a caller invokes `providers_check` with an unknown provider in the provider filter
- **THEN** the tool returns a validation error listing the supported provider names

#### Scenario: Empty provider filter
- **WHEN** a caller invokes `providers_check` with an empty provider filter
- **THEN** the tool returns a validation error explaining that at least one provider must be selected

#### Scenario: Duplicate provider filter entries
- **WHEN** a caller invokes `providers_check` with duplicate provider names in the provider filter
- **THEN** the bridge deduplicates the provider list before running checks

### Requirement: Provider readiness checks stay within aggregate budget
The system SHALL bound total `providers_check` execution time when smoke probes are requested.

#### Scenario: Default aggregate budget
- **WHEN** a caller invokes `providers_check` with `smoke: true` and no aggregate timeout override
- **THEN** the bridge uses a 110000ms aggregate timeout for the whole provider readiness call

#### Scenario: Explicit aggregate budget
- **WHEN** a caller invokes `providers_check` with `smoke: true` and `aggregateTimeoutMs`
- **THEN** the bridge uses `aggregateTimeoutMs` as the deadline for the whole provider readiness call

#### Scenario: Existing timeout remains per-provider fallback
- **WHEN** a caller invokes `providers_check` with `smoke: true`, `timeoutMs`, and no provider-specific timeout for a selected provider
- **THEN** the bridge uses `timeoutMs` as that provider's smoke budget and does not reinterpret it as the aggregate deadline

#### Scenario: Invalid aggregate timeout
- **WHEN** a caller invokes `providers_check` with `aggregateTimeoutMs` that is zero, negative, non-integer, or greater than 120000
- **THEN** the tool returns a validation error describing the accepted aggregate timeout range

#### Scenario: All-provider smoke under aggregate budget
- **WHEN** a caller invokes `providers_check` with `smoke: true` and an aggregate timeout
- **THEN** the bridge returns all completed provider results before the aggregate timeout expires

#### Scenario: Aggregate timeout expires
- **WHEN** smoke probes are still running when the aggregate timeout expires
- **THEN** the bridge terminates unfinished probes and returns timeout diagnostics for those providers

### Requirement: Provider smoke probes use provider-specific budgets
The system SHALL support provider-specific smoke budgets while retaining a safe default budget for providers without explicit overrides.

#### Scenario: Default provider budgets
- **WHEN** a caller invokes `providers_check` with `smoke: true` and no provider-specific timeout overrides
- **THEN** the bridge uses default smoke budgets of 20000ms for `codex`, 30000ms for `claude`, 45000ms for `kimi`, and 60000ms for `cursor`

#### Scenario: Invalid provider-specific budget
- **WHEN** a caller supplies a provider-specific budget that is zero, negative, non-integer, or greater than 90000
- **THEN** the tool returns a validation error identifying the invalid provider budget

#### Scenario: Invalid provider-specific budget key
- **WHEN** a caller supplies a `providerTimeoutMs` key that is not a supported provider name
- **THEN** the tool returns a validation error listing the supported provider names

#### Scenario: Slow provider succeeds within its budget
- **WHEN** a provider smoke probe takes longer than the default short timeout but completes within that provider's configured budget
- **THEN** the provider response reports `startupVerified: true`

#### Scenario: Provider exceeds its budget
- **WHEN** a provider smoke probe exceeds its provider-specific budget
- **THEN** the provider response reports `startupVerified: false` with `failureCategory: "provider_timeout"`

### Requirement: Smoke diagnostics distinguish readiness phases
The system SHALL distinguish binary availability from task-path startup readiness.

#### Scenario: Version succeeds but smoke times out
- **WHEN** a provider answers `--version` but its smoke probe times out
- **THEN** the provider response reports version availability, `probe: "version+smoke"`, `startupVerified: false`, `versionDurationMs`, `smokeDurationMs`, and timeout diagnostics

#### Scenario: Smoke succeeds
- **WHEN** a provider answers `--version` and completes a task-path smoke probe successfully
- **THEN** the provider response reports `startupVerified: true`, `versionDurationMs`, and `smokeDurationMs`

#### Scenario: Version-only timing
- **WHEN** a provider check runs with `smoke: false`
- **THEN** the provider response reports `versionDurationMs` and omits `smokeDurationMs`

### Requirement: Smoke probes exercise task path with minimal prompt
The system SHALL use a minimal non-mutating smoke prompt or provider-native smoke mode that still exercises the provider binary, environment policy, cwd handling, and output parsing path.

#### Scenario: Minimal prompt avoids normal workflow overhead
- **WHEN** a provider has a dedicated smoke prompt or smoke command builder
- **THEN** the smoke probe avoids unnecessary full task instructions, context loading, or workflow text while still requiring parseable provider output

#### Scenario: Smoke remains non-mutating
- **WHEN** a smoke probe runs for any provider
- **THEN** the prompt and command mode do not intentionally edit files or require write-capable permissions

#### Scenario: Minimal smoke fallback is reported
- **WHEN** a provider cannot safely use the minimal smoke renderer and falls back to the standard task prompt
- **THEN** the provider response reports the smoke prompt strategy so slow startup remains explainable

### Requirement: Concurrent smoke probes terminate provider process groups reliably
The system SHALL terminate and reap smoke probe process groups or provider child trees when provider-specific or aggregate deadlines expire.

#### Scenario: Version timeout kills process group
- **WHEN** a provider `--version` probe exceeds its version-check budget
- **THEN** the bridge terminates the provider process group or child tree, awaits child exit, and returns a timeout diagnostic

#### Scenario: Provider budget timeout kills process group
- **WHEN** a smoke probe exceeds its provider-specific budget
- **THEN** the bridge terminates the provider process group or child tree, awaits child exit, and returns a timeout diagnostic

#### Scenario: Aggregate timeout kills unfinished process groups
- **WHEN** the aggregate readiness deadline expires while smoke probes are still running
- **THEN** the bridge terminates all unfinished provider process groups or child trees, awaits their exits, and returns timeout diagnostics

### Requirement: Concurrent smoke probes are resource bounded
The system SHALL cap concurrent smoke probe execution.

#### Scenario: All-provider smoke concurrency cap
- **WHEN** all four providers are selected for smoke checks
- **THEN** the bridge runs no more than two smoke probes concurrently

#### Scenario: Concurrency environment fallback
- **WHEN** `AGENT_BRIDGE_SMOKE_CONCURRENCY` is set to `1`
- **THEN** the bridge runs smoke probes sequentially while preserving provider filtering, budgets, timing fields, and diagnostics

#### Scenario: Invalid concurrency environment value
- **WHEN** `AGENT_BRIDGE_SMOKE_CONCURRENCY` is empty, non-integer, zero, or negative
- **THEN** the bridge treats the value as unset and uses the default concurrency cap of two

#### Scenario: High concurrency environment value
- **WHEN** `AGENT_BRIDGE_SMOKE_CONCURRENCY` is greater than `4`
- **THEN** the bridge clamps smoke probe concurrency to four

#### Scenario: Concurrent collection is faster than sequential sum
- **WHEN** fake providers are configured with known sleep durations
- **THEN** the readiness test verifies elapsed wall-clock time is less than the sum of individual smoke durations

### Requirement: Providers check schema exposes readiness controls
The system SHALL expose every supported `providers_check` readiness control through the MCP tool input schema.

#### Scenario: New readiness inputs are advertised
- **WHEN** a client inspects the `providers_check` tool schema
- **THEN** the schema includes optional `providers`, `aggregateTimeoutMs`, and `providerTimeoutMs` fields with validation metadata matching runtime behavior
- **AND** `providerTimeoutMs` is advertised as an object whose keys are supported provider names and whose values are integer millisecond budgets from 1 through 90000

### Requirement: Smoke execution emits stderr diagnostics
The system SHALL emit bounded stderr diagnostics for smoke deadline and cancellation events without writing non-MCP bytes to stdout.

#### Scenario: Provider smoke timeout is logged
- **WHEN** a provider smoke probe times out
- **THEN** the bridge writes a bounded stderr diagnostic containing provider name, phase, elapsed time, and failure category

#### Scenario: MCP stdout remains clean
- **WHEN** smoke probes log deadline or cancellation diagnostics
- **THEN** MCP stdout still contains only JSON-RPC protocol messages

### Requirement: Default automated tests remain deterministic
The system SHALL verify provider readiness behavior with fake providers in automated tests.

#### Scenario: Slow fake provider proves budget behavior
- **WHEN** a fake provider sleeps beyond the old fixed timeout but within its configured provider budget
- **THEN** the readiness test passes and reports the provider as startup verified

#### Scenario: Live providers are not required in CI
- **WHEN** the default test suite runs
- **THEN** it does not require Claude, Cursor, Kimi, Codex credentials, network access, or keychain access
