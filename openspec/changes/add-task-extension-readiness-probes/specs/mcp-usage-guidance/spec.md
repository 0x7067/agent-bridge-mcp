## ADDED Requirements

### Requirement: Guidance explains task-extension readiness probes
The system SHALL explain that task-extension readiness probes are diagnostic evidence, not protocol task support.

#### Scenario: Guidance keeps task execution on Agent Bridge tools
- **WHEN** a client reads task-extension readiness guidance
- **THEN** the guidance states that Agent Bridge `task_*` tools remain the supported execution lifecycle.

#### Scenario: Guidance names blocked protocol task support
- **WHEN** guidance describes task-extension readiness
- **THEN** it states that `tasks/*`, `CreateTaskResult`, protocol task listing, protocol cancellation, and task notifications remain unavailable until a future implementation change.

#### Scenario: Guidance uses readiness evidence for future work
- **WHEN** guidance describes extension-capable client metadata
- **THEN** it frames that metadata as evidence for future implementation planning rather than permission to return protocol task results.
