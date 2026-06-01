# mcp-usage-guidance Specification

## Purpose
Define the MCP prompts and resources that expose Agent Bridge usage guidance to clients while keeping verification responsibility with the main caller.
## Requirements
### Requirement: Server exposes delegation prompts
The system SHALL expose MCP prompt templates that help callers use Agent Bridge for common delegation workflows.

#### Scenario: List guidance prompts
- **WHEN** a client sends `prompts/list`
- **THEN** the response includes prompts for delegation review, implementation, result inspection, and stalled task recovery.

#### Scenario: Get guidance prompt
- **WHEN** a client sends `prompts/get` for a known Agent Bridge prompt
- **THEN** the response includes user-message text that names the relevant task lifecycle tools and keeps final verification responsibility with the caller.

#### Scenario: Unknown guidance prompt
- **WHEN** a client sends `prompts/get` for an unknown prompt name
- **THEN** the server returns a JSON-RPC invalid params error.

### Requirement: Server exposes guidance resources
The system SHALL expose MCP resources containing Agent Bridge caller workflow, safety, and provider capability guidance.

#### Scenario: List guidance resources
- **WHEN** a client sends `resources/list`
- **THEN** the response includes `agent-bridge://` resources for caller workflow, safety, and provider capabilities.

#### Scenario: Read guidance resource
- **WHEN** a client sends `resources/read` for a known guidance resource URI
- **THEN** the response includes text markdown content for that exact resource.

#### Scenario: Reject non-allowlisted guidance resource
- **WHEN** a client sends `resources/read` for a malformed, non-`agent-bridge://`, or unknown resource URI
- **THEN** the server returns a JSON-RPC resource-not-found error without reading from the filesystem.

### Requirement: Guidance preserves caller responsibility
The system SHALL state in server-discoverable guidance that provider output is evidence for the main caller rather than final verification.

#### Scenario: Verification guidance
- **WHEN** a client reads guidance through prompts or resources
- **THEN** the guidance tells the caller to inspect task output and run the relevant project verification before claiming work complete.

### Requirement: Guidance exposes operator workflow prompts
The system SHALL expose MCP prompt templates for host-runner lifecycle operation and dogfood delegation workflows.

#### Scenario: List operator workflow prompts
- **WHEN** a client sends `prompts/list`
- **THEN** the response includes prompts for Claude host-runner lifecycle, provider comparison, and dogfood delegation workflows in addition to the existing delegation prompts.

#### Scenario: Read operator workflow prompt
- **WHEN** a client sends `prompts/get` for a known operator workflow prompt
- **THEN** the response includes user-message text naming the relevant Agent Bridge lifecycle tools and keeping final verification responsibility with the caller.

### Requirement: Guidance exposes operator workflow resources
The system SHALL expose MCP resources for host-runner lifecycle and reproducible dogfood workflows.

#### Scenario: List operator workflow resources
- **WHEN** a client sends `resources/list`
- **THEN** the response includes `agent-bridge://` resources for Claude host-runner lifecycle and dogfood workflows.

#### Scenario: Read host-runner lifecycle resource
- **WHEN** a client reads the host-runner lifecycle resource
- **THEN** the markdown content explains start, ping/readiness, restart after workspace-policy changes, stop, stale socket behavior, and unavailable-runner diagnostics.

#### Scenario: Read dogfood workflows resource
- **WHEN** a client reads the dogfood workflows resource
- **THEN** the markdown content describes read-only review, isolated implementation, stalled-task recovery, and provider comparison workflows using bounded waits and final `task_result` inspection.

### Requirement: Guidance points callers at review packets
The system SHALL mention `reviewPacket` in result-inspection guidance without replacing existing raw evidence.

#### Scenario: Inspect result guidance
- **WHEN** a client reads result-inspection guidance through prompts or resources
- **THEN** the guidance tells the caller to inspect `reviewPacket` along with logs, diagnostics, git status, diff, changed files, and verification output.

### Requirement: Guidance points operators to doctor
The system SHALL recommend `doctor` as the first troubleshooting step for Agent Bridge setup and readiness issues.

#### Scenario: Caller workflow guidance mentions doctor
- **WHEN** a client reads caller workflow guidance
- **THEN** the guidance tells operators to run `doctor` before deeper provider readiness or host-runner troubleshooting.

#### Scenario: Host-runner guidance mentions doctor
- **WHEN** a client reads Claude host-runner lifecycle guidance
- **THEN** the guidance tells operators to use `doctor` to inspect socket reachability and workspace-policy mismatch.

#### Scenario: Result guidance remains separate
- **WHEN** a client reads task result inspection guidance
- **THEN** the guidance keeps `doctor` separate from task-result verification and does not imply doctor verifies delegated work.

