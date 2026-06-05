## MODIFIED Requirements

### Requirement: Guidance aligns with self-guided workflow
The system SHALL keep MCP prompts and resources aligned with the initialization instructions for the standard Agent Bridge workflow.

#### Scenario: Caller workflow guidance names self-guided surfaces
- **WHEN** a client reads caller workflow guidance
- **THEN** the guidance mentions initialization instructions, structured tool results, next-action metadata, `agent_spawn`, `agents_list`, and the existing lifecycle tools.

#### Scenario: Manual lifecycle remains documented
- **WHEN** a client reads caller workflow guidance
- **THEN** prompts and resources still describe the manual lifecycle using `doctor`, `providers_check`, `agent_spawn`, `agents_list`, `task_wait`, `task_logs`, `task_transcript`, `task_result`, and `task_remove`.
- **AND** the guidance describes `task_spawn` as a legacy compatibility launch tool until a later removal change.
