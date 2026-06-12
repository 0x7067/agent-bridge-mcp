# structured-json-logging Specification

## Purpose
Define structured JSON logging and tracing spans that keep MCP stdout clean
while preserving task and provider context on stderr.
## Requirements
### Requirement: Tracing initialization targets stderr only
The system SHALL initialize a `tracing_subscriber` that formats events as JSON and writes exclusively to stderr.

#### Scenario: Server startup initializes tracing
- **WHEN** the server process begins
- **THEN** `tracing` is initialized with a JSON subscriber attached to stderr.
- **AND** no `tracing` layer writes to stdout.

#### Scenario: Stdout remains unpolluted
- **WHEN** any code path emits a `tracing::info!` or `tracing::error!` event
- **THEN** stdout contains only MCP JSON-RPC traffic with no interleaved log lines.

### Requirement: Spans carry task identity and provider context
The system SHALL attach `agent_id`, `provider`, `mode`, and `task_status` to spans crossing the actor, launcher, and drainer boundaries.

#### Scenario: Span hierarchy on spawn
- **WHEN** `agent_spawn` initiates a task
- **THEN** a root span `spawn_task` is entered with `agent_id`, `provider`, and `mode` fields.
- **AND** child spans `launch_child`, `drain_stdout`, and `drain_stderr` inherit the same `agent_id`.

#### Scenario: Completion span records outcome
- **WHEN** a task finalizes
- **THEN** a `finalize_task` span records `exit_code`, `signal`, `error_type`, and `duration_ms`.

### Requirement: Legacy eprintln statements are migrated
The system SHALL replace all `eprintln!("[agent-bridge] ...")` invocations with semantically equivalent `tracing` events.

#### Scenario: Fatal error logged as event
- **WHEN** the task actor encounters a fatal error
- **THEN** it emits a `tracing::error!` event with the same textual content and contextual span fields.

#### Scenario: Panic hook preserves termination signaling
- **WHEN** a panic occurs
- **THEN** the panic hook emits a `tracing::error!` event before sending SIGTERM to tracked children.

### Requirement: Clippy denies accidental stdout printing
The system SHALL enforce a lint that forbids `print!`, `println!`, and `eprintln!` in the main crate.

#### Scenario: CI gate catches regression
- **WHEN** a contributor adds `println!` to a source file
- **THEN** `cargo clippy --all-targets -- -D warnings` fails the build.
