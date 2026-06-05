## ADDED Requirements

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
