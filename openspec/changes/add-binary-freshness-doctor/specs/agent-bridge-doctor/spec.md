## ADDED Requirements

### Requirement: Doctor includes binary freshness diagnostics
The system SHALL include Agent Bridge binary freshness diagnostics in the `doctor` response while preserving existing setup diagnostics.

#### Scenario: Doctor response includes binary section
- **WHEN** a caller invokes `doctor`
- **THEN** the response includes a top-level `binary` section.
- **AND** the existing `summary`, `server`, `workspace`, `state`, `clients`, `taskExtensionReadiness`, `providers`, `launchReadiness`, `claudeHostRunner`, and `recommendations` sections remain present when those capabilities are active.

#### Scenario: Binary recommendations are ordered after setup blockers
- **WHEN** doctor detects setup blockers and binary freshness issues
- **THEN** setup blocker recommendations appear before binary freshness recommendations.

#### Scenario: Binary section uses nested and top-level recommendations
- **WHEN** doctor detects binary freshness issues
- **THEN** the `binary` section includes section-local recommendations.
- **AND** top-level `recommendations` includes structured binary follow-ups after setup, provider, client, and task-extension readiness recommendations.

#### Scenario: Binary diagnostics are bounded
- **WHEN** doctor inspects binary files
- **THEN** it reads only the running executable, installed binary path, and release candidate path.
- **AND** it does not read file contents beyond the configured fingerprint cap.
