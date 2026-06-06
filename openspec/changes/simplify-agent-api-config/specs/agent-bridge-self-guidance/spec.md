## ADDED Requirements

### Requirement: Self-guidance uses agent-only public terminology
The system SHALL describe the public lifecycle using `agent_*` tools and `agentId` identifiers without presenting `task_*` tools or `taskId` as public compatibility paths.

#### Scenario: Initialization instructions use canonical fields
- **WHEN** a caller reads `initialize.instructions`
- **THEN** the instructions name the canonical `agent_*` lifecycle.
- **AND** they do not instruct callers to use public `task_*` lifecycle tools or `taskId` arguments.

#### Scenario: Next actions use agentId
- **WHEN** a caller receives `nextActions` metadata for an Agent Bridge lifecycle record
- **THEN** tool-call arguments in `nextActions` use `agentId`.
- **AND** they do not include `taskId`.
