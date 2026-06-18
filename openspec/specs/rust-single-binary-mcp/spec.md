# rust-single-binary-mcp Specification

## Purpose
Define the Rust-built Agent Bridge MCP binary behavior, including stdio protocol compatibility, task lifecycle handling, provider process safety, persisted state, and release packaging expectations.
## Requirements
### Requirement: Rust binary preserves MCP public API
The system SHALL provide a Rust-built MCP server binary that preserves the current public MCP tool names, tool input schemas, argument defaults, validation behavior, response shapes, and tool error semantics while allowing documented additive response fields and additive tools.

#### Scenario: MCP protocol smoke
- **WHEN** a caller sends `initialize`, `tools/list`, `providers_list`, `providers_check`, and `agent_preview` requests over stdio to the Rust binary
- **THEN** the responses match the migrated public JSON-RPC behavior for those requests.

#### Scenario: Unknown public input
- **WHEN** a caller passes an unknown field to a tool input object
- **THEN** the Rust binary rejects the input with tool-level error semantics.

#### Scenario: Task result includes additive review packet
- **WHEN** a caller reads `agent_result`
- **THEN** the existing result fields remain present and the response may include documented additive fields such as `reviewPacket`.

#### Scenario: Doctor tool is additive
- **WHEN** a caller inspects `tools/list`
- **THEN** the response may include documented additive tools such as `doctor` without changing existing tool names or schemas.

### Requirement: Stdio transport compatibility fixtures
The system SHALL define Rust stdio tests that exercise the production binary.

#### Scenario: Fixture parity
- **WHEN** the stdio test suite runs against the Rust binary
- **THEN** the suite compares public MCP responses for protocol initialization, tool listing, provider listing, validation failures, task preview, task lifecycle states, logs, results, stale startup recovery, and worktree cleanup failures.

#### Scenario: Fixture normalization
- **WHEN** golden fixture outputs contain dynamic values such as task IDs, timestamps, durations, process IDs, environment ordering, or map key ordering
- **THEN** the fixture harness normalizes those dynamic fields while still comparing semantic response shapes, required fields, error types, caps, and command arguments.

#### Scenario: stdout discipline
- **WHEN** the Rust binary writes a response to stdout
- **THEN** stdout contains only valid newline-delimited MCP JSON-RPC messages and provider logs are never written to stdout.

#### Scenario: EOF shutdown
- **WHEN** the MCP client closes stdin
- **THEN** the Rust binary exits cleanly after completing any in-flight response handling, matching the current stdio server behavior.

#### Scenario: diagnostics stay on stderr
- **WHEN** the Rust binary logs diagnostics, reports panics, or emits tracing output
- **THEN** those diagnostics are written to stderr and never corrupt MCP stdout.

### Requirement: Type-safe Rust domain model
The system SHALL model public inputs, provider behavior, task lifecycle state, errors, and persisted records with typed Rust structures and enums.

#### Scenario: Tool input parsing
- **WHEN** the Rust binary parses tool input arguments
- **THEN** it uses typed input structures with unknown-field rejection rather than ad hoc string map inspection.

#### Scenario: Task state transitions
- **WHEN** a task moves between lifecycle states
- **THEN** the transition is represented through typed state or transition functions that reject illegal lifecycle moves.

#### Scenario: Serialized state access
- **WHEN** concurrent MCP requests inspect or mutate task state
- **THEN** the Rust task manager serializes access through an explicit actor, channel, or async lock model so registry updates and lifecycle transitions cannot race.

#### Scenario: Responsive actor
- **WHEN** provider processes, git commands, log drains, or worktree cleanup are running
- **THEN** the task manager actor remains able to process independent list, status, logs, wait, stop, and result commands by receiving completion events from background tasks rather than awaiting long-running work directly.

#### Scenario: Actor panic handling
- **WHEN** the task manager actor panics unexpectedly
- **THEN** the server fails fast rather than leaving request handlers waiting indefinitely.

### Requirement: Provider behavior parity
The system SHALL preserve the existing provider adapter contract in the Rust implementation.

#### Scenario: Provider command descriptors
- **WHEN** the Rust binary builds task commands for Claude, Cursor, Kimi, or Codex
- **THEN** command paths, arguments, cwd, timeout values, prompt rendering, and provider-specific options match the current provider adapter behavior.

#### Scenario: Provider environment policy
- **WHEN** the Rust binary builds provider process environments
- **THEN** environment allowlists and provider-specific exclusions match the current provider adapter behavior, including Claude `ANTHROPIC_BASE_URL` stripping.

### Requirement: State compatibility and migration
The system SHALL preserve inspectability of existing task state or provide an explicit migration path.

#### Scenario: Existing registry startup
- **WHEN** the Rust binary starts with an existing `registry.json`
- **THEN** it either loads the registry safely with compatible field names or performs a versioned migration that preserves inspectable completed tasks.

#### Scenario: Unknown registry fields
- **WHEN** the Rust binary reads a registry record with fields it does not use
- **THEN** it tolerates unknown fields in persisted state while still rejecting unknown fields in public tool inputs.

#### Scenario: Task ID compatibility
- **WHEN** the Rust binary creates a task
- **THEN** it uses the existing `task_` plus UUID-hex identifier shape and avoids collisions with already persisted task IDs.

#### Scenario: Stale running tasks
- **WHEN** the Rust binary starts and finds previously `queued` or `running` tasks
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

### Requirement: Process and log safety
The system SHALL preserve current provider process safety while avoiding Rust-specific pipe and shutdown regressions.

#### Scenario: Capped logs keep draining
- **WHEN** provider stdout or stderr exceeds the configured log cap
- **THEN** the Rust binary preserves capped log output while continuing to drain provider pipes so provider processes cannot block on full pipes.

#### Scenario: Invalid provider UTF-8
- **WHEN** provider stdout or stderr contains invalid UTF-8 bytes
- **THEN** the Rust binary decodes logs lossy instead of failing the task solely because of log decoding.

#### Scenario: Timeout and stop
- **WHEN** a task times out or a caller stops a running task
- **THEN** the Rust binary terminates the provider process, records the correct final status and error type, and keeps the task inspectable.

#### Scenario: Removing active tasks
- **WHEN** a caller attempts to remove a queued or running task
- **THEN** the Rust binary preserves current public behavior by rejecting removal until the task is stopped or final.

#### Scenario: Provider child cleanup on server signal
- **WHEN** the Rust MCP process receives `SIGINT` or `SIGTERM`
- **THEN** it sends termination to tracked active provider processes before exiting.

#### Scenario: Bounded shutdown cleanup
- **WHEN** provider processes ignore termination during server shutdown
- **THEN** the Rust binary waits only for a bounded cleanup deadline before escalating remaining children and continuing shutdown.

### Requirement: Final runtime is one MCP binary
The system SHALL make the final production MCP entrypoint a single built executable named `agent-bridge-mcp`.

#### Scenario: Final MCP config
- **WHEN** a user configures the MCP server after migration
- **THEN** the config can point directly at the built `agent-bridge-mcp` binary.

#### Scenario: Direct binary release path
- **WHEN** release artifacts are produced for the first Rust migration
- **THEN** direct built binaries are available for the supported targets without requiring users to compile Rust during install.

#### Scenario: External provider dependencies
- **WHEN** the Rust binary is installed
- **THEN** documentation and provider checks make clear that `git`, official interactive `claude`, `cursor-agent`, `pi`, and `codex` remain external runtime dependencies.
- **AND** `claude-p` is not required for normal Claude provider execution.

### Requirement: Packaging smoke coverage
The system SHALL verify the built or packaged artifact through stdio smoke tests before release.

#### Scenario: Built binary smoke
- **WHEN** the release candidate binary is built
- **THEN** a smoke test executes that binary and verifies `initialize`, `tools/list`, `providers_list`, `providers_check`, and `agent_preview`.

### Requirement: Rust binary exposes MCP guidance capabilities
The Rust MCP binary SHALL advertise and serve the MCP prompts and resources capabilities for static Agent Bridge usage guidance.

#### Scenario: Initialize advertises guidance capabilities
- **WHEN** a caller sends `initialize` over stdio to the Rust binary
- **THEN** the response advertises `tools`, `prompts`, and `resources` capabilities while preserving the supported protocol version.

#### Scenario: Guidance methods over stdio
- **WHEN** a caller sends `prompts/list`, `prompts/get`, `resources/list`, and `resources/read` over stdio to the Rust binary
- **THEN** the responses use MCP-compatible prompt and resource response shapes.

#### Scenario: Existing tools remain available
- **WHEN** a caller sends `tools/list` after guidance capability support is added
- **THEN** the existing provider and task lifecycle tools remain listed with their current public names.

### Requirement: Provider fatal errors finalize task lifecycle
The Rust MCP binary SHALL transition tasks to a final failed state when a provider emits unrecoverable fatal-error evidence, even if the provider process does not exit promptly.

#### Scenario: Fatal provider stderr is captured
- **WHEN** a provider emits fatal-error evidence on stderr
- **THEN** the Rust binary captures the stderr in task logs and diagnostic excerpts without writing non-MCP bytes to stdout

#### Scenario: Fatal provider evidence ends running state
- **WHEN** a running task has provider fatal-error evidence that cannot recover
- **THEN** lifecycle tools stop reporting the task as running after a bounded cleanup period
- **AND** the task remains inspectable through `agent_result`

#### Scenario: Fatal provider cleanup terminates process tree
- **WHEN** Agent Bridge finalizes a task early because of fatal provider evidence
- **THEN** it terminates and reaps the provider process group or child tree before recording the final state

#### Scenario: Fatal provider finalization keeps intended timeout behavior
- **WHEN** a provider does not emit known fatal-error evidence and exceeds its configured timeout
- **THEN** the Rust binary reports timeout classification and diagnostics that express the intended timeout contract

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

### Requirement: Rust binary negotiates task protocol support explicitly
The Rust MCP binary SHALL advertise protocol-level task support only when the selected MCP task compatibility surface is implemented and tested.

#### Scenario: No premature task capability
- **WHEN** a caller sends `initialize` before task support is implemented
- **THEN** the response does not advertise MCP task capabilities.

### Requirement: Rust binary handles task protocol methods safely
The Rust MCP binary SHALL reject unsupported protocol task methods without corrupting stdout or task state.

#### Scenario: Unsupported task method
- **WHEN** a caller sends an MCP task method that Agent Bridge does not support under the negotiated design
- **THEN** the server returns a JSON-RPC method-not-found or invalid-params error according to the compatibility design.
