## ADDED Requirements

### Requirement: Presentation exposes progress-aware next actions
The system SHALL include progress-aware metadata and next actions in agent presentation summaries so clients can render running provider agents without treating provider silence as failure.

#### Scenario: Running task recommends observe
- **WHEN** a client reads presentation metadata for a running task
- **THEN** the presentation or top-level `nextActions` includes a primary bounded `agent_observe` or `agent_wait` action with ready-to-call arguments.

#### Scenario: Presentation actions include observe
- **WHEN** a client reads action availability for a running task
- **THEN** `presentation.actions` includes an `observe` action targeting `agent_observe`.

#### Scenario: Silent task explains output cadence
- **WHEN** a running task has not emitted provider output
- **THEN** presentation metadata includes progress state explaining the expected output cadence and whether silence is still within the recommended provider budget.

#### Scenario: Stop remains explicit
- **WHEN** a task is running but still within the recommended observation budget
- **THEN** `agent_stop` remains available as an explicit lifecycle action but is not ranked ahead of observe, wait, or inspect actions.
