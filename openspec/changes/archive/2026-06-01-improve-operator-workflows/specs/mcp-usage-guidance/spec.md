## ADDED Requirements

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
