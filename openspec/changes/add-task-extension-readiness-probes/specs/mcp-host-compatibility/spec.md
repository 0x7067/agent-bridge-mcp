## ADDED Requirements

### Requirement: Host fixtures cover task-extension readiness metadata
The system SHALL include compatibility fixtures for MCP clients with and without task-extension readiness metadata.

#### Scenario: Non-task client fixture
- **WHEN** the stdio fixture initializes or calls tools without task-extension metadata
- **THEN** Agent Bridge classifies task-extension readiness as unavailable.
- **AND** existing Agent Bridge lifecycle tools remain usable.

#### Scenario: Task extension-capable client fixture
- **WHEN** the stdio fixture sends `io.modelcontextprotocol/tasks` client capability metadata
- **THEN** Agent Bridge classifies the client shape as extension-capable without advertising server task support.

#### Scenario: Legacy task metadata fixture
- **WHEN** the stdio fixture sends legacy 2025-11-25 task metadata
- **THEN** Agent Bridge classifies the client shape as legacy-only without treating it as current extension support.
