## ADDED Requirements

### Requirement: Guidance explains progress observation
The system SHALL teach callers how to observe long-running and silent provider tasks without prematurely stopping or falling back from healthy tasks.

#### Scenario: Caller workflow mentions observation
- **WHEN** a client reads caller workflow guidance
- **THEN** the guidance tells callers to use bounded observation, task status, logs, and transcripts to distinguish silence from failure.

#### Scenario: Cursor silence guidance
- **WHEN** a client reads stalled-task or provider guidance
- **THEN** the guidance explains that Cursor JSON-mode tasks may emit no stdout until final completion
- **AND** it tells callers not to stop Cursor solely because the transcript only shows a spawn event while the task is still within its configured timeout and recommended observation budget.

#### Scenario: Fallback guidance uses final evidence
- **WHEN** guidance describes fallback to another provider
- **THEN** it tells callers to fall back after a final failure, provider timeout, explicit stop decision, or exceeded observation budget rather than after a single short silent polling interval.
