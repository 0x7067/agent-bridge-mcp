## MODIFIED Requirements

### Requirement: Guidance aligns with self-guided workflow
The system SHALL keep MCP prompts and resources aligned with the initialization instructions for the standard Agent Bridge workflow.

#### Scenario: Caller workflow guidance names self-guided surfaces
- **WHEN** a client reads caller workflow guidance
- **THEN** the guidance mentions initialization instructions, structured tool results, next-action metadata, and the canonical `agent_*` tool family.

#### Scenario: Manual lifecycle remains agent-oriented
- **WHEN** a client reads caller workflow guidance
- **THEN** prompts and resources describe the manual lifecycle using `doctor`, `providers_check`, `agent_preview`, `agent_spawn`, `agent_list`, `agent_observe`, `agent_status`, `agent_logs`, `agent_transcript`, `agent_result`, and `agent_remove`.
- **AND** the guidance does not present a parallel `task_*` workflow for normal MCP clients.
