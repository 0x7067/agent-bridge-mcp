# client-config-doctor Specification

## Purpose
TBD - created by archiving change add-client-config-doctor. Update Purpose after archive.
## Requirements
### Requirement: Doctor reports MCP client configuration diagnostics
The system SHALL report read-only diagnostics for supported MCP client configuration surfaces.

#### Scenario: Default client diagnostics
- **WHEN** a caller invokes `doctor`
- **THEN** the response includes a keyed `clients` object with `codex`, `claude`, and `cursor` entries.
- **AND** each entry includes the client name, config path, config presence, parse status, Agent Bridge registration status, command diagnostics, verification status, and recommendations.

#### Scenario: Client diagnostic field names
- **WHEN** a caller reads a client diagnostic entry
- **THEN** the entry uses camelCase fields named `client`, `status`, `configPath`, `configPresent`, `parseStatus`, `registrationStatus`, `command`, `args`, `envKeys`, `verificationStatus`, `verificationCommands`, and `recommendations`.

#### Scenario: Client diagnostic status values
- **WHEN** client diagnostics classify a supported client
- **THEN** the per-client `status` value is one of `ok`, `info`, `warning`, or `error`.
- **AND** missing config files and absent registrations are classified as `info` rather than top-level setup blockers.

#### Scenario: Missing client config
- **WHEN** a supported client's expected config file is missing
- **THEN** the corresponding client diagnostic reports the config as missing and does not report Agent Bridge as registered.

#### Scenario: Parse failure
- **WHEN** a supported client's config file exists but cannot be parsed for the expected format
- **THEN** the corresponding client diagnostic reports a parse error without panicking.
- **AND** the diagnostic does not expose raw config content.

### Requirement: Client diagnostics detect Agent Bridge registrations
The system SHALL detect the `agent-bridge` MCP server registration in supported client configuration files.

#### Scenario: Codex registration detected
- **WHEN** `~/.codex/config.toml` contains an `[mcp_servers.agent-bridge]` registration
- **THEN** the Codex client diagnostic reports Agent Bridge as registered and includes command diagnostics derived from that section.

#### Scenario: Claude registration detected
- **WHEN** `~/.claude.json` contains an `mcpServers.agent-bridge` registration
- **THEN** the Claude client diagnostic reports Agent Bridge as registered and includes command diagnostics derived from that object.

#### Scenario: Cursor registration detected
- **WHEN** `~/.cursor/mcp.json` contains an `mcpServers.agent-bridge` registration
- **THEN** the Cursor client diagnostic reports Agent Bridge as registered and includes command diagnostics derived from that object.

#### Scenario: Registration absent
- **WHEN** a supported client's config file exists but does not contain an Agent Bridge registration
- **THEN** the corresponding client diagnostic reports Agent Bridge as absent and recommends adding the registration through that client.

#### Scenario: Similar registration detected
- **WHEN** a supported client's config contains another MCP server entry whose command appears to reference `agent-bridge-mcp`
- **THEN** the diagnostic reports the exact `agent-bridge` registration as absent.
- **AND** the diagnostic may include a non-blocking similar-registration note without treating it as the canonical registration.

### Requirement: Client diagnostics validate command shape without leaking secrets
The system SHALL validate the registered command shape and redact sensitive configuration values.

#### Scenario: Command path exists
- **WHEN** a registered Agent Bridge command is an absolute path that exists
- **THEN** the corresponding client diagnostic reports command status as ok.

#### Scenario: Command path missing
- **WHEN** a registered Agent Bridge command is an absolute path that does not exist
- **THEN** the corresponding client diagnostic reports a command warning and recommends inspecting the configured command path.

#### Scenario: Command resolved through PATH
- **WHEN** a registered Agent Bridge command is not an absolute path
- **THEN** the corresponding client diagnostic reports that the command requires PATH resolution rather than asserting it exists.

#### Scenario: Command missing
- **WHEN** a registered Agent Bridge config does not contain a command string
- **THEN** the corresponding client diagnostic reports a command warning.

#### Scenario: Environment values are present
- **WHEN** a registered Agent Bridge config contains environment values
- **THEN** the diagnostic reports environment key names or redacted indicators only.
- **AND** raw token, key, OAuth, auth, password, or secret values are not included in the response.

### Requirement: Client diagnostics provide verification guidance
The system SHALL provide structured follow-up verification guidance without claiming client startup success from static config inspection.

#### Scenario: Codex verification command
- **WHEN** Codex client diagnostics are returned
- **THEN** the diagnostic includes a shell follow-up command `["codex", "mcp", "list"]` when Agent Bridge appears registered.

#### Scenario: Claude verification command
- **WHEN** Claude client diagnostics are returned
- **THEN** the diagnostic includes a shell follow-up command `["claude", "mcp", "list"]` when Agent Bridge appears registered.

#### Scenario: Cursor file-only verification
- **WHEN** Cursor client diagnostics are returned
- **THEN** the diagnostic reports static file inspection as not verified and does not claim runtime MCP startup success.

#### Scenario: Top-level shell recommendation shape
- **WHEN** doctor adds a top-level recommendation for a client verification command
- **THEN** the recommendation uses `kind: "shell"` and a `command` array rather than an MCP `tool` call.

#### Scenario: Doctor remains read-only
- **WHEN** a caller invokes `doctor`
- **THEN** client diagnostics do not edit config files, execute client verification commands, spawn provider tasks, create task records, or create managed worktrees.

### Requirement: Client diagnostics stay bounded to user-level config files
The system SHALL inspect only the expected user-level MCP client config files in this capability.

#### Scenario: Bounded config inspection
- **WHEN** doctor gathers client diagnostics
- **THEN** it reads only `~/.codex/config.toml`, `~/.claude.json`, and `~/.cursor/mcp.json`.
- **AND** it does not recursively search the home directory or project directories.

