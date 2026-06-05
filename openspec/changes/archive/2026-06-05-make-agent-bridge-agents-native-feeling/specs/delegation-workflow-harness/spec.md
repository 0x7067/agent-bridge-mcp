## ADDED Requirements

### Requirement: Delegation workflow documents task presentation
The system SHALL document how clients should use Agent Bridge `presentation` metadata on task lifecycle responses to render delegated provider tasks as native-feeling agents while preserving the raw lifecycle workflow for automation and debugging.

#### Scenario: Native-client workflow is discoverable
- **WHEN** an operator reads project documentation or MCP guidance
- **THEN** the guidance describes the native-client path for listing task presentation summaries, inspecting status, reading results, stopping running tasks, and cleaning up final tasks.

#### Scenario: Raw lifecycle workflow remains documented
- **WHEN** an operator reads project documentation or MCP guidance
- **THEN** the guidance still documents the lower-level preview, spawn, wait, logs, transcript, result, stop, and remove workflow.

### Requirement: Delegation workflow explains unavailable interactive controls
The system SHALL document how clients should present reply and resume controls for providers or tasks that do not support interactive continuation.

#### Scenario: Reply is unavailable
- **WHEN** provider capability metadata or task action availability reports reply as unsupported
- **THEN** workflow guidance tells clients to present the action with `state: "unavailable"` and an explanation rather than treating it as a failed tool call.

#### Scenario: Resume is unavailable
- **WHEN** provider capability metadata or task action availability reports resume as unsupported
- **THEN** workflow guidance tells clients to present the action with `state: "unavailable"` and an explanation and to use a new task when continuation is required.

### Requirement: Delegation workflow prioritizes active and recent tasks
The system SHALL document that native clients should prioritize active and recent Agent Bridge tasks over the full historical registry.

#### Scenario: Client opens agent drawer
- **WHEN** a client opens a native Agent Bridge agent list or drawer
- **THEN** workflow guidance directs it to show active tasks and recent final tasks first.

#### Scenario: Operator needs historical records
- **WHEN** an operator needs older completed task records
- **THEN** workflow guidance points to filtered or raw registry inspection rather than defaulting the native presentation to all historical tasks.
