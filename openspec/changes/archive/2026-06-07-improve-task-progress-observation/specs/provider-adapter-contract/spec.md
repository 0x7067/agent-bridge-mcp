## ADDED Requirements

### Requirement: Provider adapters expose output cadence metadata
The system SHALL expose provider-specific output cadence and observation guidance through provider capability metadata.

#### Scenario: Provider list includes cadence metadata
- **WHEN** a caller invokes `providers_list`
- **THEN** each provider includes `outputCadence` metadata with `cadence`, `firstOutputExpected`, `recommendedPollMs`, `recommendedSilentBudgetMs`, `fallbackAfterMs`, `advisory`, and `note` fields.

#### Scenario: Cursor cadence is final-output aware
- **WHEN** a caller reads Cursor provider metadata
- **THEN** the metadata identifies Cursor's JSON-mode output as final-output-oriented with `cadence: "final_json"` and provides a conservative recommended observation budget before fallback or manual stop.

#### Scenario: Cadence metadata is advisory
- **WHEN** provider output cadence metadata is returned
- **THEN** the metadata is advisory and does not mark a provider as launchable, healthy, verified, or failed.
