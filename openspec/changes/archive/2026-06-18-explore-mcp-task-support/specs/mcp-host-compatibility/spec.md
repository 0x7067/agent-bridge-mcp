## ADDED Requirements

### Requirement: Host compatibility covers task metadata readiness
The system SHALL test Agent Bridge behavior for MCP hosts that send current, legacy, unknown, conflicting, or absent task metadata.

#### Scenario: Non-task client compatibility
- **WHEN** a client initializes without task capabilities
- **THEN** Agent Bridge remains usable through existing tools and does not require task-augmented requests.

#### Scenario: Current task extension metadata
- **WHEN** a client initializes with `io.modelcontextprotocol/tasks` extension metadata
- **THEN** Agent Bridge reports task-extension readiness diagnostics without advertising protocol tasks.

#### Scenario: Legacy metadata is ignored safely
- **WHEN** a client sends task-related metadata that is not part of the negotiated protocol surface
- **THEN** Agent Bridge ignores or rejects it according to the compatibility design without passing protocol metadata into public tool arguments.

#### Scenario: Raw task metadata is not leaked
- **WHEN** a client sends task metadata through request `_meta`
- **THEN** Agent Bridge readiness diagnostics do not include raw task metadata values.
