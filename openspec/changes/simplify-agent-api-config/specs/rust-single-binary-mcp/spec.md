## ADDED Requirements

### Requirement: Agent lifecycle uses canonical agent identifiers
The Rust MCP binary SHALL expose `agentId` as the only public lifecycle identifier field for advertised `agent_*` tools.

#### Scenario: Lifecycle schemas require agentId
- **WHEN** a caller sends `tools/list`
- **THEN** `agent_status`, `agent_wait`, `agent_logs`, `agent_transcript`, `agent_observe`, `agent_result`, `agent_stop`, and `agent_remove` schemas require `agentId`.
- **AND** those schemas do not expose `taskId`.

#### Scenario: Lifecycle responses return agentId
- **WHEN** a caller invokes an `agent_*` lifecycle tool that returns an agent record, result, transcript, observation, stop result, or remove result
- **THEN** the response uses `agentId` for the lifecycle identifier.
- **AND** the response does not include `taskId`.

#### Scenario: Legacy taskId is rejected
- **WHEN** a caller passes `taskId` to an advertised `agent_*` lifecycle tool
- **THEN** the tool returns an unknown-argument error.

#### Scenario: New identifiers use agent prefix
- **WHEN** `agent_spawn` creates a new lifecycle record
- **THEN** the returned `agentId` starts with `agent_`.

### Requirement: Default state directory is unambiguous
The Rust MCP binary SHALL use one default state directory across runtime task storage and diagnostics.

#### Scenario: Runtime and doctor use same default
- **WHEN** `AGENT_BRIDGE_STATE_DIR` is unset
- **THEN** the task manager stores lifecycle state under `~/.agent-bridge-mcp/state`.
- **AND** `doctor.state.path` reports `~/.agent-bridge-mcp/state` after home expansion.

#### Scenario: State override remains explicit
- **WHEN** `AGENT_BRIDGE_STATE_DIR` is set
- **THEN** both runtime task storage and `doctor.state.path` use that explicit path.

## MODIFIED Requirements

### Requirement: Rust binary preserves MCP public API
The system SHALL provide a Rust-built MCP server binary that preserves the canonical public MCP tool names, strict input validation behavior, response shapes, and tool error semantics for the simplified `agent_*` API while allowing documented additive response fields and additive tools.

#### Scenario: MCP protocol smoke
- **WHEN** a caller sends `initialize`, `tools/list`, `providers_list`, `providers_check`, and `agent_preview` requests over stdio to the Rust binary
- **THEN** the responses match the canonical public JSON-RPC behavior for those requests.

#### Scenario: Unknown public input
- **WHEN** a caller passes an unknown field to a tool input object
- **THEN** the Rust binary rejects the input with tool-level error semantics.

#### Scenario: Agent result includes additive review packet
- **WHEN** a caller reads `agent_result`
- **THEN** the existing canonical result fields remain present and the response may include documented additive fields such as `reviewPacket`.

#### Scenario: Doctor tool is additive
- **WHEN** a caller inspects `tools/list`
- **THEN** the response may include documented additive tools such as `doctor` without changing canonical tool names or schemas.

### Requirement: State compatibility and migration
The system SHALL preserve inspectability of simplified agent state or fail startup with a clear diagnostic when state was serialized with an incompatible pre-simplification public shape.

#### Scenario: Existing simplified registry startup
- **WHEN** the Rust binary starts with an existing `registry.json` serialized with `agentId` records
- **THEN** it loads the registry safely with compatible field names.

#### Scenario: Unknown registry fields
- **WHEN** the Rust binary reads a registry record with fields it does not use
- **THEN** it tolerates unknown fields in persisted state while still rejecting unknown fields in public tool inputs.

#### Scenario: Agent ID compatibility
- **WHEN** the Rust binary creates an agent
- **THEN** it uses the `agent_` plus UUID-hex identifier shape and avoids collisions with already persisted agent IDs.

#### Scenario: Legacy taskId registry startup
- **WHEN** the Rust binary starts with a registry serialized with old `taskId` lifecycle fields
- **THEN** it fails startup with a clear registry diagnostic instead of silently migrating or exposing mixed task/agent identifiers.

#### Scenario: Stale running agents
- **WHEN** the Rust binary starts and finds previously `queued` or `running` agents
- **THEN** it marks them `failed_stale` with the existing stale error semantics.

#### Scenario: Atomic write temp cleanup
- **WHEN** the Rust binary starts after a crash during registry persistence
- **THEN** it removes or ignores known temporary registry files before loading canonical registry state.

#### Scenario: Same-directory atomic registry writes
- **WHEN** the Rust binary persists `registry.json`
- **THEN** it writes temporary registry files in the same directory as the canonical registry file before atomically renaming them into place.

#### Scenario: Corrupted registry startup
- **WHEN** the Rust binary starts with a present but invalid canonical `registry.json`
- **THEN** it fails startup with a clear diagnostic instead of silently replacing existing state with an empty registry.

### Requirement: Packaging smoke coverage
The system SHALL verify the built or packaged artifact through stdio smoke tests before release.

#### Scenario: Built binary smoke
- **WHEN** the release candidate binary is built
- **THEN** a smoke test executes that binary and verifies `initialize`, `tools/list`, `providers_list`, `providers_check`, and `agent_preview`.
