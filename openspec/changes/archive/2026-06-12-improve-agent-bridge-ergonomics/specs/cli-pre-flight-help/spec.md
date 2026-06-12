# cli-pre-flight-help Specification

## Purpose
Define the standalone CLI surface for the `agent-bridge-mcp` binary so operators can introspect, validate, and pre-flight the server without booting an MCP client.

## ADDED Requirements

### Requirement: Binary responds to standard CLI flags
The system SHALL parse `--help`, `--version`, and `--config-check` via `clap` before entering the MCP stdio loop.

#### Scenario: Help flag
- **WHEN** an operator executes `agent-bridge-mcp --help`
- **THEN** the binary prints usage information, available subcommands, and env var hints to stdout and exits with code 0.

#### Scenario: Version flag
- **WHEN** an operator executes `agent-bridge-mcp --version`
- **THEN** the binary prints the crate version and exits with code 0.

#### Scenario: Config check flag
- **WHEN** an operator executes `agent-bridge-mcp --config-check`
- **THEN** the binary loads the layered configuration, validates workspace root existence, and prints a terse JSON summary to stdout indicating validity or specific errors.

### Requirement: Doctor smoke can be triggered from CLI
The system SHALL expose a `--doctor-smoke` flag that runs the provider readiness smoke suite synchronously and prints a JSON report.

#### Scenario: CLI smoke succeeds
- **WHEN** an operator executes `agent-bridge-mcp --doctor-smoke`
- **AND** all registered providers respond to smoke probes within timeout budgets
- **THEN** the binary prints a JSON report with `status: ok` and exits with code 0.

#### Scenario: CLI smoke fails
- **WHEN** an operator executes `agent-bridge-mcp --doctor-smoke`
- **AND** one or more providers fail to respond
- **THEN** the binary prints a JSON report with `status: error`, includes per-provider diagnostics, and exits with a non-zero code.

### Requirement: Stdio server remains the default invocation
The system SHALL enter the MCP stdio loop when invoked with no recognized flags or with the default subcommand.

#### Scenario: Plain invocation
- **WHEN** the binary is executed with no arguments
- **THEN** it behaves identically to today's `main_entry()`, listening on stdin and writing JSON-RPC to stdout.
