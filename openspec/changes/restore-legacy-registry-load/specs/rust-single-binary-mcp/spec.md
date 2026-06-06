## MODIFIED Requirements

### Requirement: State compatibility and migration
The system SHALL preserve inspectability of existing task state through compatible reads or an explicit migration path.

#### Scenario: Existing registry startup
- **WHEN** the Rust binary starts with an existing `registry.json`
- **THEN** it either loads the registry safely with compatible field names or performs a versioned migration that preserves inspectable completed tasks.

#### Scenario: Legacy taskId registry startup
- **WHEN** the Rust binary starts with a registry serialized with old `taskId` and `taskDir` persisted fields
- **THEN** it loads the registry by normalizing those persisted fields to the current typed record shape.
- **AND** it exposes the records through public lifecycle responses using `agentId` fields.
- **AND** it does not accept `taskId` as a public lifecycle input argument.

#### Scenario: Doctor validates typed registry compatibility
- **WHEN** a caller invokes `doctor` against a state directory containing a registry
- **THEN** doctor validates the registry with the same typed compatibility parser used by lifecycle startup.

#### Scenario: Unknown registry fields
- **WHEN** the Rust binary reads a registry record with fields it does not use
- **THEN** it tolerates unknown fields in persisted state while still rejecting unknown fields in public tool inputs.

#### Scenario: Agent ID compatibility
- **WHEN** the Rust binary creates a new agent
- **THEN** it uses the `agent_` plus UUID-hex identifier shape and avoids collisions with already persisted IDs.

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
