## ADDED Requirements

### Requirement: Guidance distinguishes skills from Agent Bridge runtime
The system SHALL explain the boundary between provider skills and Agent Bridge MCP lifecycle tools in server-discoverable guidance.

#### Scenario: Caller reads workflow guidance
- **WHEN** a client reads Agent Bridge workflow guidance through prompts or resources
- **THEN** the guidance explains that provider skills document direct CLI runbooks while Agent Bridge tools provide MCP-native delegation, readiness checks, task lifecycle state, logs, diffs, and result inspection

#### Scenario: Caller reads provider capability guidance
- **WHEN** a client reads provider capability guidance
- **THEN** the guidance names the relevant provider skill for direct CLI usage details without duplicating the full provider CLI runbook

#### Scenario: Caller chooses isolated implementation
- **WHEN** guidance describes write-capable delegated implementation
- **THEN** it recommends Agent Bridge `task_spawn` with managed worktree isolation rather than direct provider skill invocation

### Requirement: Guidance routes direct provider troubleshooting to skills
The system SHALL route direct provider CLI troubleshooting and flag usage to provider skills while retaining Agent Bridge setup troubleshooting in MCP guidance.

#### Scenario: Provider CLI flag question
- **WHEN** guidance discusses direct provider CLI flags, output format options, or one-shot subprocess usage
- **THEN** it points the operator to the relevant provider skill

#### Scenario: Agent Bridge setup question
- **WHEN** guidance discusses workspace policy, state directory, provider readiness checks, Claude host-runner setup, task logs, task results, or managed worktree cleanup
- **THEN** it keeps the operator in the Agent Bridge MCP workflow

#### Scenario: Provider appears misconfigured
- **WHEN** guidance describes a provider that is missing, timing out, or failing startup readiness through Agent Bridge
- **THEN** it recommends `doctor`, `providers_check`, and `task_preview` before falling back to direct provider skill troubleshooting

### Requirement: Guidance keeps verification with the main caller
The system SHALL preserve existing verification responsibility while adding provider skill references.

#### Scenario: Direct provider skill result
- **WHEN** guidance references a direct provider skill invocation
- **THEN** it states that the main caller still inspects output and runs the smallest relevant proof before claiming work complete

#### Scenario: Agent Bridge result
- **WHEN** guidance references an Agent Bridge task result
- **THEN** it continues to require inspection of `reviewPacket`, logs, diagnostics, git status, diff, changed files, and verification output
