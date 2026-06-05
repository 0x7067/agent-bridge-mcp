## ADDED Requirements

### Requirement: Delegation workflow distinguishes native and protocol tasks
The system SHALL document when callers should use Agent Bridge lifecycle tools versus protocol-level MCP task support.

#### Scenario: Default workflow uses Agent Bridge task tools
- **WHEN** a caller reads the standard delegation workflow
- **THEN** the workflow continues to use Agent Bridge `task_*` tools as the primary stable lifecycle.

#### Scenario: Protocol task workflow is conditional
- **WHEN** guidance describes MCP task support
- **THEN** it states that protocol task support depends on negotiated host/client capabilities and may be unavailable.

#### Scenario: Verification boundary remains unchanged
- **WHEN** guidance describes either task workflow
- **THEN** it states that provider output remains evidence and project verification remains caller-owned.
