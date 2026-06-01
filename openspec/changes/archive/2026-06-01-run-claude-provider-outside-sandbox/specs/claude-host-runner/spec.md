## ADDED Requirements

### Requirement: Claude host runner executes only structured Claude-p requests
The system SHALL provide an opt-in host runner that executes Claude provider commands outside the Codex sandbox without accepting arbitrary command or shell descriptor execution.

#### Scenario: Host runner accepts a valid structured Claude-p request
- **WHEN** the MCP server sends a structured Claude-p request whose protocol version, request type, workspace policy id, cwd, timeout, mode, model, effort, and prompt payload pass host-runner validation
- **THEN** the host runner reconstructs the `claude-p` command from its own hardcoded template, executes it outside the Codex sandbox, and returns captured stdout, stderr, truncation flags, exit status or signal, elapsed time, and failure category metadata.

#### Scenario: Host runner rejects non-Claude provider requests
- **WHEN** the host runner receives a request type for Cursor, Kimi, Codex, or an unknown provider
- **THEN** it rejects the request without spawning a process.

#### Scenario: Host runner rejects command descriptor requests
- **WHEN** the host runner receives a request containing a command string, shell script, arbitrary argv, or executable path to run
- **THEN** it rejects the request without spawning a process.

### Requirement: Claude host runner preserves workspace safety
The system SHALL enforce the same workspace and cwd safety policy for host-runner execution that the MCP server enforces for normal task execution.

#### Scenario: Host runner starts without workspace configuration
- **WHEN** the host runner starts without `AGENT_BRIDGE_WORKSPACES` or with an empty workspace list
- **THEN** the host runner fails startup before binding its socket.

#### Scenario: Host runner starts with non-canonicalizable workspace
- **WHEN** the host runner starts with any workspace path that cannot be canonicalized
- **THEN** the host runner fails startup before binding its socket.

#### Scenario: Host runner receives cwd under configured workspace
- **WHEN** a host-runner request contains a cwd that canonicalizes under `AGENT_BRIDGE_WORKSPACES`
- **THEN** the host runner may execute the request if all other validations pass.

#### Scenario: Host runner receives mismatched workspace policy id
- **WHEN** a host-runner request contains a workspace policy id that does not match the runner's startup workspace policy id
- **THEN** the host runner rejects the request without spawning a process.

#### Scenario: Host runner receives cwd outside configured workspace
- **WHEN** a host-runner request contains a cwd that canonicalizes outside `AGENT_BRIDGE_WORKSPACES`
- **THEN** the host runner rejects the request without spawning a process.

#### Scenario: Host runner receives symlink escape cwd
- **WHEN** a host-runner request contains a cwd path that appears under a workspace but canonicalizes outside the workspace through a symlink
- **THEN** the host runner rejects the request without spawning a process.

### Requirement: Claude host runner output is isolated and bounded
The system SHALL keep host-runner provider output out of MCP protocol stdout and return bounded diagnostics through the existing provider result channels.

#### Scenario: Host-runner provider emits stdout and stderr
- **WHEN** the host-runner child process writes stdout or stderr
- **THEN** those bytes are captured into the provider result or task logs and are never written to MCP server stdout.

#### Scenario: Host-runner provider emits excessive stdout or stderr
- **WHEN** the host-runner child process writes more than the configured per-stream byte cap
- **THEN** the host runner uses bounded streaming capture that never stores more than the per-stream cap plus small accounting metadata, truncates excess bytes, marks the corresponding truncation flag in the response, and continues enforcing the task timeout.

#### Scenario: Host-runner provider times out
- **WHEN** the host-runner child process exceeds the configured timeout
- **THEN** the host runner terminates the child process group where supported and returns a timeout failure category with bounded stdout and stderr excerpts.

#### Scenario: Host-runner client disconnects while provider is running
- **WHEN** the host-runner client disconnects before a running Claude child process exits
- **THEN** the host runner terminates and reaps the child process.

#### Scenario: Host-runner logs lifecycle events
- **WHEN** the host runner writes its own stderr logs
- **THEN** those logs contain only stable error codes, coarse categories, and timing metadata and do not contain prompts, token values, env values, full cwd paths, workspace paths, socket paths, command arguments, stdout, or stderr.

#### Scenario: Host-runner provider returns secrets in output
- **WHEN** host-runner output contains prompt text or allowlisted token values known to the MCP server
- **THEN** diagnostics returned through MCP responses redact those values before exposing them to callers.

### Requirement: Claude host runner setup is explicit
The system SHALL make host-runner use explicit in configuration, previews, provider checks, and documentation.

#### Scenario: Host runner socket directory is not owner-only
- **WHEN** the host runner cannot create or validate its socket directory as user-owned with `0700` permissions
- **THEN** the host runner fails startup before binding its socket.

#### Scenario: Host runner socket is created
- **WHEN** the host runner binds its Unix socket
- **THEN** the socket is created inside the validated owner-only socket directory and uses `0600` socket file permissions where the platform exposes socket filesystem permissions.

#### Scenario: Host runner socket path is unsafe
- **WHEN** the configured socket path is a symlink, regular file, or under a configured workspace directory
- **THEN** the host runner fails startup before binding its socket.

#### Scenario: Host runner finds stale socket
- **WHEN** the configured socket path already exists as a socket and a connection probe confirms that no runner is listening
- **THEN** the host runner may remove the stale socket and bind a new socket.

#### Scenario: Host runner finds live socket
- **WHEN** the configured socket path already exists as a socket and a connection probe succeeds
- **THEN** the host runner fails startup without unlinking the socket.

#### Scenario: Host runner is configured
- **WHEN** `AGENT_BRIDGE_CLAUDE_HOST_SOCKET` is configured for the MCP server
- **THEN** Claude task previews and provider diagnostics identify that Claude will use the host runner.

#### Scenario: Configured host runner is unavailable
- **WHEN** `AGENT_BRIDGE_CLAUDE_HOST_SOCKET` is configured but the socket cannot be reached or the runner reports a protocol mismatch
- **THEN** Claude task execution and smoke checks fail with an actionable host-runner diagnostic and do not silently fall back to sandboxed Claude execution.

#### Scenario: Host runner is not configured
- **WHEN** `AGENT_BRIDGE_CLAUDE_HOST_SOCKET` is not configured
- **THEN** the system preserves the existing direct Claude execution path for deterministic tests and non-host-runner environments.

### Requirement: Claude host runner protocol is versioned and inspectable
The system SHALL use a versioned local protocol with deterministic framing for host-runner requests and responses.

#### Scenario: Host runner receives valid protocol version
- **WHEN** a request contains the current host-runner protocol version and one newline-delimited JSON request object
- **THEN** the host runner parses the request and returns one newline-delimited JSON response object.

#### Scenario: Host runner receives oversized request
- **WHEN** a client sends more than the configured maximum request-line bytes before a newline
- **THEN** the host runner rejects the request without continuing to buffer and without spawning a process.

#### Scenario: Host runner receives unsupported protocol version
- **WHEN** a request contains an unsupported host-runner protocol version
- **THEN** the host runner rejects the request without spawning a process and returns a protocol mismatch error.

#### Scenario: Host runner receives ping request
- **WHEN** a request contains the current protocol version and request type `ping`
- **THEN** the host runner returns an ok response with runner version, protocol version, workspace policy id, and readiness metadata.

#### Scenario: Host runner returns error
- **WHEN** a request fails protocol, workspace, cwd, validation, timeout, or spawn handling
- **THEN** the host runner returns an error object with a stable `code` field and human-readable `message`.

#### Scenario: Host runner error message contains diagnostics
- **WHEN** the host runner returns an error message
- **THEN** the message is sanitized and does not contain prompts, token values, env values, full cwd paths, workspace paths, socket paths, command arguments, stdout, or stderr.

### Requirement: Claude host runner workspace policy id is deterministic
The system SHALL derive the workspace policy id from the runner's configured workspace list using deterministic canonicalization.

#### Scenario: Workspace policy id is computed
- **WHEN** the system computes a workspace policy id
- **THEN** it canonicalizes each configured workspace to an absolute path, sorts the canonical paths bytewise, and joins them with a NUL byte.

#### Scenario: Workspace policy cannot be canonicalized
- **WHEN** any configured workspace cannot be canonicalized
- **THEN** the component computing the workspace policy id fails closed before accepting Claude host-runner work.

### Requirement: Claude host runner handles concurrent requests safely
The system SHALL handle concurrent host-runner connections without one running Claude process blocking acceptance of unrelated requests.

#### Scenario: Multiple host-runner clients connect
- **WHEN** multiple clients connect to the host-runner socket while a Claude request is running
- **THEN** the host runner accepts each connection and processes each valid request independently within bounded task execution.

#### Scenario: Host runner receives shutdown signal
- **WHEN** the host runner receives SIGTERM or SIGINT while child processes are active
- **THEN** it stops accepting new connections, terminates and reaps active child processes, and exits.
