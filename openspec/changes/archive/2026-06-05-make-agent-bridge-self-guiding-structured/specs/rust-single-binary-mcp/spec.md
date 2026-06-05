## ADDED Requirements

### Requirement: Rust binary returns initialization instructions
The Rust MCP binary SHALL return Agent Bridge initialization instructions without breaking existing initialization behavior.

#### Scenario: Initialize includes instructions and existing capabilities
- **WHEN** a caller sends `initialize` over stdio to the Rust binary
- **THEN** the response includes `instructions`.
- **AND** the response still advertises the existing `tools`, `prompts`, and `resources` capabilities.

#### Scenario: Existing initialize clients remain compatible
- **WHEN** a caller ignores unknown additive initialize fields
- **THEN** the caller can continue using the existing Agent Bridge MCP tools.

### Requirement: Rust binary emits structured tool results compatibly
The Rust MCP binary SHALL include structured JSON content for JSON-returning tools while preserving text content compatibility.

#### Scenario: Structured content over stdio
- **WHEN** the stdio compatibility harness calls a JSON-returning Agent Bridge tool
- **THEN** the response includes `structuredContent` with the same semantic payload as the serialized text content.

#### Scenario: Output schema fixtures
- **WHEN** the stdio compatibility harness inspects `tools/list`
- **THEN** stable JSON tools expose output schemas and existing input schemas remain strict.

### Requirement: Protocol fixtures cover next-action metadata
The Rust MCP binary SHALL include deterministic compatibility coverage for task next-action metadata.

#### Scenario: Running task next action fixture
- **WHEN** a deterministic fake-provider task is running
- **THEN** stdio tests verify the returned presentation next action points to an inspectable lifecycle step.

#### Scenario: Final managed worktree next action fixture
- **WHEN** a deterministic managed-worktree task is final and uninspected
- **THEN** stdio tests verify result inspection is recommended before cleanup.
