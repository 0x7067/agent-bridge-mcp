## MODIFIED Requirements

### Requirement: Rust binary preserves MCP public API
The system SHALL provide a Rust-built MCP server binary that preserves the current public MCP tool names, tool input schemas, argument defaults, validation behavior, response shapes, and tool error semantics while allowing documented additive response fields.

#### Scenario: MCP protocol smoke
- **WHEN** a caller sends `initialize`, `tools/list`, `providers_list`, `providers_check`, and `task_preview` requests over stdio to the Rust binary
- **THEN** the responses match the migrated public JSON-RPC behavior for those requests.

#### Scenario: Unknown public input
- **WHEN** a caller passes an unknown field to a tool input object
- **THEN** the Rust binary rejects the input with tool-level error semantics.

#### Scenario: Task result includes additive review packet
- **WHEN** a caller reads `task_result`
- **THEN** the existing result fields remain present and the response may include documented additive fields such as `reviewPacket`.
