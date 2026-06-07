# agent-bridge-doctor Delta Specification

## Purpose
Additive extension of the `agent-bridge-doctor` capability to support CLI-triggered pre-flight smoke checks without altering the MCP tool contract.

## ADDED Requirements

### Requirement: Doctor smoke can be invoked from CLI
The system SHALL expose a `--doctor-smoke` CLI flag that exercises the same provider readiness engine as the MCP `doctor` tool with `smoke: true`.

#### Scenario: CLI smoke mirrors MCP tool behavior
- **WHEN** an operator runs `agent-bridge-mcp --doctor-smoke`
- **THEN** the binary performs the same version, probe, and aggregate timeout logic as the MCP `doctor` tool.
- **AND** the printed JSON schema matches the `doctor` tool's `providers` and `launchReadiness` structures.

#### Scenario: CLI smoke respects provider filtering
- **WHEN** an operator runs `agent-bridge-mcp --doctor-smoke --provider claude --provider codex`
- **THEN** the binary evaluates only Claude and Codex readiness.

#### Scenario: CLI smoke does not mutate state
- **WHEN** `--doctor-smoke` executes
- **THEN** no task records are created, no registry mutations occur, and no provider tasks are spawned.
