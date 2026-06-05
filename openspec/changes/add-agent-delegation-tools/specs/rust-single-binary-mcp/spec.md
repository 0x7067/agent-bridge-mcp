## MODIFIED Requirements

### Requirement: Rust binary exposes MCP tools
The Rust MCP binary SHALL expose Agent Bridge provider, doctor, and canonical agent lifecycle tools through the MCP `tools/list` and `tools/call` surfaces.

#### Scenario: Tools list includes agent delegation tools
- **WHEN** a caller inspects `tools/list`
- **THEN** the response includes `agent_preview`, `agent_spawn`, `agent_list`, `agent_status`, `agent_wait`, `agent_logs`, `agent_transcript`, `agent_observe`, `agent_result`, `agent_stop`, and `agent_remove`.

#### Scenario: Parallel task lifecycle is not advertised
- **WHEN** a caller inspects `tools/list`
- **THEN** the response does not include public `task_*` lifecycle tools.
