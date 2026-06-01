# mcp-usage-guidance Specification

## Purpose
TBD - created by archiving change expose-mcp-usage-guidance. Update Purpose after archive.
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

