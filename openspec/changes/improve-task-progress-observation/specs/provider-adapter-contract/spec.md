## ADDED Requirements

### Requirement: Provider adapters expose output cadence metadata
The system SHALL expose provider-specific output cadence and observation guidance through provider capability metadata.

#### Scenario: Provider list includes cadence metadata
- **WHEN** a caller invokes `providers_list`
- **THEN** each provider includes output cadence metadata describing whether task output is expected incrementally, at final JSON completion, or provider-dependent.

#### Scenario: Cursor cadence is final-output aware
- **WHEN** a caller reads Cursor provider metadata
- **THEN** the metadata identifies Cursor's JSON-mode output as final-output-oriented and provides a conservative recommended observation budget before fallback or manual stop.

#### Scenario: Cadence metadata is advisory
- **WHEN** provider output cadence metadata is returned
- **THEN** the metadata is advisory and does not mark a provider as launchable, healthy, verified, or failed.
