## MODIFIED Requirements

### Requirement: Providers check schema exposes readiness controls
The system SHALL expose every supported provider readiness control through the `doctor`
tool input schema, since `doctor` is the single public readiness entry point. The readiness
engine behavior (filtering, aggregate budget, provider-specific budgets, smoke phases, and
stderr diagnostics) is unchanged; only the advertised tool name moves from `providers_check`
to `doctor`.

#### Scenario: Readiness inputs are advertised on doctor
- **WHEN** a client inspects the `doctor` tool schema
- **THEN** the schema includes optional `focus`, `smoke`, `providers`, `aggregateTimeoutMs`,
  and `providerTimeoutMs` fields with validation metadata matching runtime behavior
- **AND** `providerTimeoutMs` is advertised as an object whose keys are supported provider
  names and whose values are integer millisecond budgets from 1 through 90000.

#### Scenario: Standalone providers_check is not advertised
- **WHEN** a client inspects `tools/list`
- **THEN** no `providers_check` tool is advertised and the readiness controls are reachable
  through `doctor` with `focus: "providers"`.
