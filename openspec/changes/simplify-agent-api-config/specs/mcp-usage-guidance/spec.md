## ADDED Requirements

### Requirement: Usage guidance documents the simplified API and configuration
The system SHALL document the simplified Agent Bridge public API as `agent_*` tools with `agentId` identifiers and a single default state directory.

#### Scenario: Guidance resources name agentId
- **WHEN** a client reads Agent Bridge prompts or guidance resources
- **THEN** lifecycle examples use `agentId` for follow-up tool calls.
- **AND** they do not present `taskId` as a supported public argument.

#### Scenario: Guidance documents minimal configuration
- **WHEN** a client reads setup or caller workflow guidance
- **THEN** the guidance documents `AGENT_BRIDGE_WORKSPACES` as the required workspace policy input.
- **AND** it documents `~/.agent-bridge-mcp/state` as the default state directory when `AGENT_BRIDGE_STATE_DIR` is omitted.

#### Scenario: Migration notes are explicit
- **WHEN** a user reads project documentation for this breaking change
- **THEN** the documentation tells callers to rename `taskId` reads and arguments to `agentId`.
- **AND** it states that public `taskId` compatibility is not provided.
