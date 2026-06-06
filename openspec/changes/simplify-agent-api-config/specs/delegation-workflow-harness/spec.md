## ADDED Requirements

### Requirement: Delegation workflow uses canonical agent identifiers
The documented delegation workflow SHALL use only canonical Agent Bridge lifecycle names and identifiers.

#### Scenario: Workflow cleanup uses agent_remove with agentId
- **WHEN** workflow guidance describes final result cleanup
- **THEN** it tells callers to inspect `agent_result` and then call `agent_remove` with `agentId`.
- **AND** it does not instruct callers to call `task_remove` or pass `taskId`.

#### Scenario: Stalled workflow uses agent tools
- **WHEN** workflow guidance describes stalled-agent recovery
- **THEN** it names bounded `agent_observe`, `agent_wait`, `agent_logs`, `agent_status`, and final `agent_result` inspection.
