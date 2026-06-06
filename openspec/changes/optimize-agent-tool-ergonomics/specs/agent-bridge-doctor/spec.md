## MODIFIED Requirements

### Requirement: Doctor aggregates provider readiness
The system SHALL include provider availability checks in doctor output and SHALL expose a
`focus` selector so doctor is the single public entry point for both full setup diagnostics
and focused provider readiness, subsuming the standalone `providers_check` tool.

#### Scenario: Focused provider readiness
- **WHEN** a caller invokes `doctor` with `focus: "providers"`
- **THEN** doctor runs only the provider readiness section (the former `providers_check` use
  case) using the same readiness engine, validation, and deduplication semantics
- **AND** it accepts the same `smoke`, `providers`, `aggregateTimeoutMs`, and
  `providerTimeoutMs` controls.

#### Scenario: Full diagnostics by default
- **WHEN** a caller invokes `doctor` without `focus` or with `focus: "all"`
- **THEN** doctor reports the full setup, workspace, state, client, binary, host-runner, and
  provider readiness sections.

#### Scenario: Default provider checks avoid smoke probes
- **WHEN** a caller invokes `doctor` without `smoke`
- **THEN** doctor runs provider version checks and reports `startupVerified: false` unless
  existing readiness logic proves otherwise.

#### Scenario: Optional smoke probes
- **WHEN** a caller invokes `doctor` with `smoke: true`
- **THEN** doctor runs the bounded provider smoke readiness behavior previously reached
  through `providers_check`.

#### Scenario: Provider filtering
- **WHEN** a caller passes a provider filter
- **THEN** doctor checks only the selected providers using the same validation and
  deduplication semantics previously documented for `providers_check`.

#### Scenario: Missing provider binary
- **WHEN** a selected provider binary is unavailable
- **THEN** doctor reports a provider warning or error and recommends installing or
  configuring that provider binary.
