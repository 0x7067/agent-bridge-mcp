## ADDED Requirements

### Requirement: Doctor includes client configuration diagnostics
The system SHALL include supported MCP client configuration diagnostics in the `doctor` response while preserving existing server, workspace, state, provider, and Claude host-runner diagnostics.

#### Scenario: Doctor response includes clients section
- **WHEN** a caller invokes `doctor`
- **THEN** the response includes a top-level `clients` section.
- **AND** the existing `summary`, `server`, `workspace`, `state`, `providers`, `launchReadiness`, `claudeHostRunner`, and `recommendations` sections remain present.

#### Scenario: Client issues affect recommendations
- **WHEN** doctor detects missing, unparseable, absent, or invalid Agent Bridge client registrations
- **THEN** doctor includes concise recommendations for those client configuration issues.

#### Scenario: Client diagnostics do not affect summary status
- **WHEN** client diagnostics detect missing config files, absent Agent Bridge registrations, parse failures, or command warnings
- **THEN** those client diagnostics do not change `summary.status`.
- **AND** top-level setup triage remains based on existing workspace, state, provider, launch readiness, and Claude host-runner diagnostics.

#### Scenario: Client diagnostics are bounded
- **WHEN** doctor reads supported client configuration files
- **THEN** it reads only the expected Codex, Claude, and Cursor user-level config files.
- **AND** it does not recursively search the home directory.

#### Scenario: Client diagnostics do not prove provider work
- **WHEN** doctor reports client diagnostics as ok or warning
- **THEN** it does not claim delegated task output, provider model behavior, or project tests are verified.
