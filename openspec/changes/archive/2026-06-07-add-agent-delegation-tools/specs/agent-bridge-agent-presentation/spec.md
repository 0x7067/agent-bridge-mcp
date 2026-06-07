## ADDED Requirements

### Requirement: Agent-oriented tools expose native presentation
The system SHALL expose a single agent-oriented MCP tool family for launching, listing, observing, inspecting, stopping, and cleaning up provider agents without requiring clients to use public `task_*` verbs.

#### Scenario: List active and recent agents
- **WHEN** a client invokes `agent_list` with no arguments
- **THEN** the response includes an `agents` array ordered with active tasks first and recent final tasks second
- **AND** each agent includes the existing compact `presentation` summary and `nextActions`
- **AND** the default result remains bounded.

#### Scenario: Filter listed agents
- **WHEN** a client invokes `agent_list` with filters for status, provider, mode, workspace, title text, or limit
- **THEN** the response includes only matching agent summaries up to the requested bound
- **AND** limits above 100 are rejected.

#### Scenario: Agent list remains presentation-first
- **WHEN** a client invokes `agent_list` with `presentation` or `scope`
- **THEN** the request is rejected as an unsupported argument
- **AND** raw or full-history registry inspection is not exposed as a separate public MCP tool in the canonical workflow.

#### Scenario: Spawn provider agent
- **WHEN** a client invokes `agent_spawn` with provider, mode, prompt, and optional launch arguments
- **THEN** the server starts the provider task through the existing task manager
- **AND** the response includes the persisted lifecycle identifier used by `agent_status`, `agent_wait`, `agent_logs`, `agent_transcript`, `agent_result`, `agent_stop`, and `agent_remove`.

#### Scenario: Single public lifecycle tool family
- **WHEN** a client inspects `tools/list`
- **THEN** the advertised lifecycle tools use the canonical `agent_*` namespace
- **AND** parallel public `task_*` lifecycle tools are not listed.

#### Scenario: Presentation actions use canonical tools
- **WHEN** a client reads presentation action availability for a running or final agent
- **THEN** action tool names use the canonical `agent_*` namespace.

#### Scenario: Full history is not a separate public tool
- **WHEN** a client needs active or recent agent summaries
- **THEN** `agent_list` provides a bounded presentation-first list
- **AND** full task-registry history is not exposed through a separate advertised public MCP tool.
