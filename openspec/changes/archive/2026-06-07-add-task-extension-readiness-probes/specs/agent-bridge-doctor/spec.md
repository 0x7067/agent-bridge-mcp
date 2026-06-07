## ADDED Requirements

### Requirement: Doctor reports task-extension readiness diagnostics
The system SHALL include additive task-extension readiness diagnostics in the `doctor` response without changing existing setup health semantics.

#### Scenario: Doctor includes task-extension readiness
- **WHEN** a caller invokes `doctor`
- **THEN** the response includes a top-level `taskExtensionReadiness` section.
- **AND** the existing `summary`, `server`, `workspace`, `state`, `clients`, `providers`, `launchReadiness`, `claudeHostRunner`, and `recommendations` sections remain present.

#### Scenario: Task-extension readiness does not affect summary status
- **WHEN** task-extension readiness is classified as `unavailable`, `extension_capable`, `legacy_only`, `unknown`, or `unsupported`
- **THEN** the classification does not change `summary.status`.

#### Scenario: Task-extension readiness stays diagnostic
- **WHEN** `doctor` reports task-extension readiness
- **THEN** the section includes `serverAdvertisesTasks: false`.
- **AND** the report does not claim `tasks/*`, `CreateTaskResult`, protocol task listing, protocol cancellation, or task notifications are available.
