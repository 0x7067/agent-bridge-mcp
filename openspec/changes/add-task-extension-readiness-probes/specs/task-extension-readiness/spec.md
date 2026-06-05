## ADDED Requirements

### Requirement: Task extension readiness is diagnostic only
The system SHALL classify MCP task-extension readiness without advertising or implementing protocol-level MCP task support.

#### Scenario: Server does not advertise task support
- **WHEN** a caller initializes Agent Bridge after readiness probing is added
- **THEN** the initialize response does not advertise protocol-level task capabilities.
- **AND** Agent Bridge continues to use existing `task_*` tools for task execution.

#### Scenario: Readiness report is not execution support
- **WHEN** Agent Bridge reports task-extension readiness
- **THEN** the report includes `serverAdvertisesTasks: false`.
- **AND** the report does not include `tasks/*` method availability.

### Requirement: Task extension readiness classifies client metadata
The system SHALL classify task-related client metadata into stable readiness states.

#### Scenario: No task metadata
- **WHEN** a client request has no task-related capability metadata
- **THEN** readiness is classified as `unavailable`.

#### Scenario: Current task extension metadata
- **WHEN** a client request declares `io.modelcontextprotocol/tasks` extension metadata
- **THEN** readiness is classified as `extension_capable`.
- **AND** the diagnostic report records the observed extension identifier.

#### Scenario: Legacy task metadata
- **WHEN** a client request uses legacy 2025-11-25 task capability or request metadata
- **THEN** readiness is classified as `legacy_only`.
- **AND** the diagnostic report states that legacy metadata does not unblock the current task-extension implementation.

#### Scenario: Unknown task-like metadata
- **WHEN** a client request includes unknown task-like metadata
- **THEN** readiness is classified as `unknown`.
- **AND** ordinary Agent Bridge tool behavior remains unchanged.

### Requirement: Readiness probes avoid task side effects
The system SHALL keep task-extension readiness probing side-effect free.

#### Scenario: Probe does not create tasks
- **WHEN** a caller runs a task-extension readiness probe
- **THEN** no task records, logs, transcripts, managed worktrees, or provider processes are created.

#### Scenario: Unsupported protocol task methods remain unsupported
- **WHEN** a caller invokes a `tasks/*` method before protocol task support is implemented
- **THEN** Agent Bridge returns the existing JSON-RPC unsupported-method behavior.
