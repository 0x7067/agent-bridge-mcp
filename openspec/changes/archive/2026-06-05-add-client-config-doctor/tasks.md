## 1. Diagnostic Model And Parsing

- [x] 1.1 Add client diagnostic data structures for Codex, Claude, and Cursor with status, config path, parse status, registration status, command diagnostics, verification status, env key reporting, and recommendations.
- [x] 1.2 Implement JSON config parsing for Claude `~/.claude.json` and Cursor `~/.cursor/mcp.json` `mcpServers.agent-bridge` entries.
- [x] 1.3 Implement targeted Codex config scanning for `[mcp_servers.agent-bridge]` in `~/.codex/config.toml`.
- [x] 1.4 Implement command diagnostics for absolute existing paths, absolute missing paths, and PATH-resolved commands.
- [x] 1.5 Implement redacted env-key reporting without exposing raw secret values.
- [x] 1.6 Support deterministic tests through isolated HOME/config fixture paths without adding a public MCP argument.

## 2. Doctor Integration

- [x] 2.1 Add a `clients` section to `doctor` responses.
- [x] 2.2 Add client configuration recommendations to the existing doctor recommendations list.
- [x] 2.3 Update the doctor output schema to include the additive `clients` section.
- [x] 2.4 Ensure client diagnostics are read-only and do not run verification commands or spawn tasks.
- [x] 2.5 Keep client diagnostics out of top-level `summary.status` aggregation.

## 3. Documentation And Guidance

- [x] 3.1 Update README doctor documentation to describe client config diagnostics and verification boundaries.
- [x] 3.2 Update MCP guidance resources or prompts so callers know how to use `doctor.clients`.
- [x] 3.3 Document that this capability inspects user-level client config files only, not project-level overrides.

## 4. Tests

- [x] 4.1 Add unit tests for missing, absent, malformed, and registered Codex config diagnostics.
- [x] 4.2 Add unit tests for Claude and Cursor JSON config diagnostics.
- [x] 4.3 Add unit tests for command path diagnostics and env redaction.
- [x] 4.4 Add protocol or doctor integration tests proving `doctor` includes `clients` and preserves existing sections.
- [x] 4.5 Add tests proving client diagnostic issues do not alter `summary.status` and that top-level workspace/state recommendations remain ordered before client hints.

## 5. Verification

- [x] 5.1 Run `cargo test`.
- [x] 5.2 Run `cargo fmt --check`.
- [x] 5.3 Run `cargo clippy --all-targets -- -D warnings`.
- [x] 5.4 Run `openspec validate add-client-config-doctor --strict`.
