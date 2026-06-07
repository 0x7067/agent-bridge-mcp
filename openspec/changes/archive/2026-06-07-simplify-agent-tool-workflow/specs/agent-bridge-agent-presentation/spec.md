## ADDED Requirements

### Requirement: Presentation actions keep primary workflow ranked ahead of diagnostics
The system SHALL expose presentation actions in an order that makes the compact workflow obvious without hiding diagnostic actions.

#### Scenario: Running presentation actions prioritize observe
- **WHEN** a client reads presentation or `nextActions` metadata for a running agent
- **THEN** observation is ranked ahead of simple wait, raw logs, status, transcript, and stop controls.
- **AND** diagnostic and control actions remain available with safety state and reasons when applicable.

#### Scenario: Final presentation actions prioritize result inspection
- **WHEN** a client reads presentation or `nextActions` metadata for a final uninspected agent
- **THEN** result inspection is ranked ahead of cleanup.
- **AND** cleanup is marked unsafe or unavailable when managed worktree cleanup requires final result inspection.

#### Scenario: Presentation does not require raw evidence by default
- **WHEN** a client uses presentation summaries for active or recent agents
- **THEN** the summary exposes enough phase, status, progress, result availability, and verification-boundary metadata for native rendering without requiring raw logs or transcripts on the default path.
