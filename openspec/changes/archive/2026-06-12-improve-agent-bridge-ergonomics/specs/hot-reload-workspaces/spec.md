# hot-reload-workspaces Specification

## Purpose
Allow runtime refresh of the workspace root policy without restarting the MCP server process, giving operators flexibility in dynamic development environments.

## ADDED Requirements

### Requirement: Reload subcommand triggers workspace revalidation
The system SHALL expose a `reload` subcommand that sends a signal to the running server causing it to re-read the effective configuration and re-canonicalize workspace roots.

#### Scenario: Operator reloads after adding a workspace
- **WHEN** an operator executes `agent-bridge-mcp reload`
- **AND** the server PID is discoverable via a lock file or well-known socket
- **THEN** the server refreshes workspace roots and applies them to subsequent `agent_spawn` validations.

#### Scenario: Reload expands the workspace set
- **WHEN** the refreshed configuration includes additional workspace roots
- **THEN** those roots are immediately honored for new tasks.

#### Scenario: Reload shrinks the workspace set
- **WHEN** the refreshed configuration removes a previously configured workspace root
- **THEN** the server retains the old root for any active tasks referencing it.
- **AND** the server rejects new tasks targeting the removed root.

### Requirement: Graceful degradation on reload failure
The system SHALL keep the previous workspace set intact if the reload encounters unreadable files, invalid TOML, or inaccessible paths.

#### Scenario: Broken config during reload
- **WHEN** a reload is triggered but the config file contains malformed TOML
- **THEN** the server logs an error, preserves the current workspace roots, and continues serving.

### Requirement: Lock file coordination for reload
The system SHALL write a PID lock file to the state directory so the `reload` subcommand knows which process to signal.

#### Scenario: Lock file present
- **WHEN** the server starts successfully
- **THEN** it writes its PID to `~/.agent-bridge-mcp/state/server.pid`.

#### Scenario: Concurrent server start
- **WHEN** a second server tries to start and discovers an alive PID in the lock file
- **THEN** the second server refuses to start and advises the operator to stop the existing instance.
