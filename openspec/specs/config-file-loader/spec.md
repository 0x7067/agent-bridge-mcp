# config-file-loader Specification

## Purpose
Define layered Agent Bridge configuration loading from the default TOML file,
legacy environment variables, and CLI overrides.
## Requirements
### Requirement: Config file is discovered and parsed
The system SHALL locate a configuration file at `~/.agent-bridge-mcp/config.toml` and deserialize it into a typed `Config` struct.

#### Scenario: Config file exists and is valid
- **WHEN** the server starts and `~/.agent-bridge-mcp/config.toml` exists with valid TOML
- **THEN** the system loads `workspaces`, `state_dir`, `claude_host_socket`, and `max_active_tasks` from the file.

#### Scenario: Config file is missing
- **WHEN** the server starts and the config file does not exist
- **THEN** the system proceeds using defaults and environment variables without error.

#### Scenario: Config file has malformed TOML
- **WHEN** the config file exists but contains invalid TOML syntax
- **THEN** the system emits a clear error to stderr and exits with a non-zero status code.

### Requirement: Configuration layers obey strict precedence
The system SHALL resolve each configuration key using the precedence: compiled defaults < config file < environment variable < CLI flag.

#### Scenario: Env overrides file
- **WHEN** `config.toml` sets `workspaces = "/tmp/a"` and `AGENT_BRIDGE_WORKSPACES` is set to `/tmp/b`
- **THEN** the effective workspace root is `/tmp/b`.

#### Scenario: CLI flag overrides env
- **WHEN** `AGENT_BRIDGE_WORKSPACES` is set and a CLI flag `--workspaces` is supplied
- **THEN** the CLI value takes precedence.

### Requirement: Home-directory expansion is centralized
The system SHALL expand leading `~` and `~user` paths in all configuration values before validation, in a single utility function.

#### Scenario: State dir with tilde
- **WHEN** `state_dir` resolves to `"~/.agent-bridge-mcp/state"`
- **THEN** the system expands it to the invoking user's home directory before creating directories.

#### Scenario: Workspace roots with tilde
- **WHEN** a workspace root contains `~` shorthand
- **THEN** the system expands it canonically before enforcing the workspace policy.

### Requirement: Deprecated env vars warn but continue
The system SHALL recognize legacy environment variables and emit a deprecation warning while still honoring them.

#### Scenario: AGENT_BRIDGE_WORKSPACES continues to work
- **WHEN** only the legacy env var is set and no config file or CLI override exists
- **THEN** the system uses the env var value and prints a one-line stderr notice recommending the config file.
