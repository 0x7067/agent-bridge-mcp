## ADDED Requirements

### Requirement: Agent-oriented tools expose native presentation
The system SHALL expose agent-oriented MCP tools for launching provider agents and listing active or recent provider agents without requiring clients to use the lower-level spawn/list task verbs for the common path.

#### Scenario: List active and recent agents
- **WHEN** a client invokes `agents_list` with no arguments
- **THEN** the response includes an `agents` array ordered with active tasks first and recent final tasks second
- **AND** each agent includes the existing compact `presentation` summary and `nextActions`
- **AND** the default result remains bounded.

#### Scenario: Filter listed agents
- **WHEN** a client invokes `agents_list` with filters for status, provider, mode, workspace, title text, or limit
- **THEN** the response includes only matching agent summaries up to the requested bound
- **AND** limits above 100 are rejected.

#### Scenario: Agent list remains presentation-first
- **WHEN** a client invokes `agents_list` with `presentation` or `scope`
- **THEN** the request is rejected as an unsupported argument
- **AND** raw or full-history registry inspection remains available through `task_list`.

#### Scenario: Spawn provider agent
- **WHEN** a client invokes `agent_spawn` with provider, mode, prompt, and optional launch arguments
- **THEN** the server starts the provider task through the existing task manager
- **AND** the response includes the task lifecycle identifier used by the existing status, wait, logs, transcript, result, stop, and remove tools.

#### Scenario: Legacy task spawn remains during migration
- **WHEN** a client inspects `tools/list`
- **THEN** `task_spawn` remains available as a legacy compatibility launch tool until a later removal change.
