## Why

Agent Bridge is now commonly registered across Codex, Claude, and Cursor, but setup failures still require operators to know each client's separate config file and status command. A diagnostic-only client config doctor gives callers one source of truth for whether Agent Bridge appears registered, loadable, and verifiable in those client surfaces without mutating user configuration.

## What Changes

- Add structured client configuration diagnostics for Codex, Claude, and Cursor.
- Detect the expected user-level config files, parse their relevant MCP server entries, and report whether an `agent-bridge` registration is present.
- Validate the configured command path and required environment shape without exposing secrets.
- Include structured follow-up command guidance when a client has a reliable verification command, such as `codex mcp list` or `claude mcp list`, without executing those commands.
- Surface client config diagnostics through the existing `doctor` tool and guidance docs.
- Keep the capability read-only: no config edits, installs, or task spawns.

## Capabilities

### New Capabilities

- `client-config-doctor`: Covers read-only diagnostics for Codex, Claude, and Cursor MCP client configuration, including file discovery, parsing, registration detection, command validation, secret redaction, and follow-up verification guidance.

### Modified Capabilities

- `agent-bridge-doctor`: Doctor output must include client configuration diagnostics and recommendations without changing existing setup, provider, state, or host-runner checks.

## Impact

- Affected code: doctor result assembly and config parsing helpers in `crates/agent-bridge-mcp/src`.
- Affected APIs: additive `clients` diagnostics section in `doctor` responses plus related structured recommendations.
- Affected docs/specs: README and guidance resources describing client config diagnostics and verification boundaries.
- Dependencies: no new third-party dependency expected.
