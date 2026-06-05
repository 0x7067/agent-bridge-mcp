## MODIFIED Requirements

### Requirement: Rust binary exposes MCP tools
The Rust MCP binary SHALL expose Agent Bridge provider, doctor, agent, and task lifecycle tools through the MCP `tools/list` and `tools/call` surfaces.

#### Scenario: Tools list includes agent delegation tools
- **WHEN** a caller inspects `tools/list`
- **THEN** the response includes `agent_spawn` and `agents_list`
- **AND** the response still includes the existing task lifecycle tools.

#### Scenario: Legacy launch tool remains available
- **WHEN** a caller inspects `tools/list`
- **THEN** the response includes `task_spawn` as a legacy compatibility tool until a later removal change.
