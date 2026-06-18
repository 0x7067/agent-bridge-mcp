## MODIFIED Requirements

### Requirement: Final runtime is one MCP binary
The system SHALL make the final production MCP entrypoint a single built executable named `agent-bridge-mcp`.

#### Scenario: Final MCP config
- **WHEN** a user configures the MCP server after migration
- **THEN** the config can point directly at the built `agent-bridge-mcp` binary.

#### Scenario: Direct binary release path
- **WHEN** release artifacts are produced for the first Rust migration
- **THEN** direct built binaries are available for the supported targets without requiring users to compile Rust during install.

#### Scenario: External provider dependencies
- **WHEN** the Rust binary is installed
- **THEN** documentation and provider checks make clear that `git`, official interactive `claude`, `cursor-agent`, `pi`, and `codex` remain external runtime dependencies.
- **AND** `claude-p` is not required for normal Claude provider execution.
