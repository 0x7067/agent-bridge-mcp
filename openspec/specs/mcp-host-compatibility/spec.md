# mcp-host-compatibility Specification

## Purpose
Define compatibility requirements for real MCP hosts, including reserved metadata handling and production-binary fixtures that mirror Codex-style tool calls.
## Requirements
### Requirement: MCP tool calls accept envelope `_meta`
The system SHALL accept MCP-reserved `_meta` on the `tools/call` request params envelope while preserving normal tool dispatch behavior.

#### Scenario: Tool call includes metadata
- **WHEN** a caller invokes `tools/call` with params containing `name`, `arguments`, and `_meta`
- **THEN** the server dispatches the requested tool as if `name` and `arguments` had been supplied without `_meta`

#### Scenario: Metadata is not interpreted as tool input
- **WHEN** a caller includes `_meta` on the `tools/call` params envelope
- **THEN** the server does not pass `_meta` into the selected tool's public argument object

### Requirement: Public tool arguments remain strict
The system SHALL continue to reject unknown fields inside each tool's public `arguments` object.

#### Scenario: Unknown argument remains rejected
- **WHEN** a caller invokes `agent_spawn` with `dryRun: true` and an unsupported argument such as `maxTurns` inside `arguments`
- **THEN** the tool response is an error that identifies the unknown argument

#### Scenario: Envelope metadata does not disable argument validation
- **WHEN** a caller invokes `agent_spawn` with `dryRun: true`, `_meta` on the envelope, and an unsupported field inside `arguments`
- **THEN** the tool response is still an unknown-argument error

### Requirement: Production-binary compatibility fixture covers real host shape
The system SHALL include deterministic compatibility coverage that exercises the production MCP binary with real-host `tools/call` params.

#### Scenario: Codex-style tool call fixture
- **WHEN** the stdio compatibility harness sends a `tools/call` request containing `name`, `arguments`, and `_meta` to the production binary
- **THEN** the response is a valid MCP tool result for the selected tool

#### Scenario: Compatibility fixture requires no live provider credentials
- **WHEN** the compatibility fixture runs in the default test suite
- **THEN** it uses deterministic fake provider configuration and does not require live Claude, Cursor, Kimi, Codex, or network access

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
