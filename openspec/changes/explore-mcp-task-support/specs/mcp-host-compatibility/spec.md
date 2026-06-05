## ADDED Requirements

### Requirement: Host compatibility covers task-capable and non-task clients
The system SHALL test Agent Bridge behavior for MCP hosts that advertise task support and hosts that do not.

#### Scenario: Non-task client compatibility
- **WHEN** a client initializes without task capabilities
- **THEN** Agent Bridge remains usable through existing tools and does not require task-augmented requests.

#### Scenario: Task-capable client compatibility
- **WHEN** a client initializes with the task capabilities selected by the compatibility design
- **THEN** Agent Bridge exposes only the task behavior it implements and tests for that client shape.

#### Scenario: Legacy metadata is ignored safely
- **WHEN** a client sends task-related metadata that is not part of the negotiated protocol surface
- **THEN** Agent Bridge ignores or rejects it according to the compatibility design without passing protocol metadata into public tool arguments.
