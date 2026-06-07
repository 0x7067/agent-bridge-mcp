## MODIFIED Requirements

### Requirement: Rust binary preserves MCP public API
The system SHALL provide a Rust-built MCP server binary whose public surface is a
consolidated set of eight tools — `providers_list`, `doctor`, `agent_spawn`,
`agent_observe`, `agent_result`, `agent_list`, `agent_stop`, and `agent_remove` — with
strict input schemas, argument defaults, validation behavior, response shapes, and tool
error semantics, allowing documented additive response fields and additive tools. The
binary SHALL NOT advertise `providers_check`, `agent_preview`, `agent_status`,
`agent_wait`, `agent_logs`, or `agent_transcript`; their behavior is reachable through
subsuming parameters on the retained tools.

#### Scenario: MCP protocol smoke
- **WHEN** a caller sends `initialize`, `tools/list`, `providers_list`, `doctor`, and
  `agent_spawn` with `dryRun: true` requests over stdio to the Rust binary
- **THEN** the responses match the public JSON-RPC behavior for those requests
- **AND** `tools/list` advertises exactly the eight consolidated tools.

#### Scenario: Removed tools are not advertised
- **WHEN** a caller inspects `tools/list`
- **THEN** the response does not include `providers_check`, `agent_preview`,
  `agent_status`, `agent_wait`, `agent_logs`, or `agent_transcript`.

#### Scenario: Subsuming parameters are advertised
- **WHEN** a caller inspects the input schemas for `agent_spawn`, `agent_observe`,
  `agent_result`, and `doctor`
- **THEN** `agent_spawn` exposes `dryRun`, `agent_observe` exposes `until` and accepts
  `limit: 0`, `agent_result` exposes `sections` with log pagination, and `doctor` exposes
  `focus`.

#### Scenario: Unknown public input
- **WHEN** a caller passes an unknown field to a tool input object
- **THEN** the Rust binary rejects the input with tool-level error semantics.

#### Scenario: Task result includes additive review packet
- **WHEN** a caller reads `agent_result`
- **THEN** the result fields remain present and the response may include documented
  additive fields such as `reviewPacket`.

### Requirement: Packaging smoke coverage
The system SHALL verify the built or packaged artifact through stdio smoke tests before
release.

#### Scenario: Built binary smoke
- **WHEN** the release candidate binary is built
- **THEN** a smoke test executes that binary and verifies `initialize`, `tools/list`,
  `providers_list`, `doctor`, and `agent_spawn` with `dryRun: true`.
