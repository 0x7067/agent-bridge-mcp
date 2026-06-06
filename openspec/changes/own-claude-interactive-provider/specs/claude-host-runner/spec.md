## MODIFIED Requirements

### Requirement: Claude host runner executes only structured Claude requests
The system SHALL provide an opt-in host runner that executes owned interactive Claude provider requests outside the Codex sandbox without accepting arbitrary command or shell descriptor execution.

#### Scenario: Host runner accepts a valid structured Claude request
- **WHEN** the MCP server sends a structured Claude request whose protocol version, request type, workspace policy id, cwd, timeout, mode, model, effort, and prompt payload pass host-runner validation
- **THEN** the host runner executes the owned interactive Claude runner using the official `claude` CLI and returns the protocol v2 structured result with bounded PTY excerpt diagnostics, transcript diagnostics, truncation flags, exit status or signal, elapsed time, and failure category metadata.

#### Scenario: Host runner accepts protocol v2 request schema
- **WHEN** the MCP server sends a request with `version: 2`, `requestType: "claude_interactive"`, a valid workspace policy id, cwd, mode, prompt, and timeout
- **THEN** the host runner validates it against `protocol-v2.md`.
- **AND** optional `model`, `effort`, `bareProfile`, and `smokeToken` fields are accepted only when their values are valid.

#### Scenario: Host runner rejects non-Claude provider requests
- **WHEN** the host runner receives a request type for Cursor, Kimi, Codex, or an unknown provider
- **THEN** it rejects the request without spawning a process.

#### Scenario: Host runner rejects command descriptor requests
- **WHEN** the host runner receives a request containing a command string, shell script, arbitrary argv, or executable path to run
- **THEN** it rejects the request without spawning a process.

#### Scenario: Host runner rejects protocol v2 forbidden fields
- **WHEN** the host runner receives a Claude request containing `command`, `shell`, `script`, `argv`, `executablePath`, or another caller-supplied execution descriptor
- **THEN** it rejects the request with `protocol_rejected` without spawning a process.

### Requirement: Claude host runner uses owned-runner protocol versioning
The system SHALL bump and validate the host-runner protocol when switching from structured `claude-p` execution to owned interactive Claude execution.

#### Scenario: Host runner receives owned-runner protocol
- **WHEN** the MCP server sends an owned-runner Claude request with the current protocol version
- **THEN** the host runner accepts the request if all validation passes.
- **AND** the current protocol version for owned-runner Claude requests is `2`.

#### Scenario: Host runner receives legacy protocol
- **WHEN** the host runner receives a legacy structured `claude-p` request or unsupported protocol version
- **THEN** it rejects the request with `protocol_mismatch` without spawning a process.

#### Scenario: Host runner returns owned-runner result
- **WHEN** an owned interactive Claude request completes
- **THEN** the response includes structured fields for exit status or signal, duration, failure category, bounded PTY output excerpts, truncation flags, Stop payload metadata, StopFailure metadata when present, and transcript parse diagnostics.

#### Scenario: Owned-runner result schema
- **WHEN** the host runner returns an owned-runner result
- **THEN** the result includes `exitCode`, `signal`, `durationMs`, `failureCategory`, `ptyOutputExcerpt`, `ptyOutputTruncated`, `stop`, `stopFailure`, and `transcript` fields where applicable.
- **AND** `stop` contains bounded Stop payload metadata, `stopFailure` contains bounded StopFailure metadata, and `transcript` contains parse status and bounded diagnostics without raw prompt text.

#### Scenario: MCP receives owned-runner result
- **WHEN** the MCP server receives a protocol v2 owned-runner result
- **THEN** it maps `result.finalText` into the existing task result surface on success.
- **AND** it maps `failureCategory`, `transcript`, `stop`, `stopFailure`, and bounded PTY diagnostics into provider diagnostics on failure.
- **AND** it does not use legacy print-mode stdout JSON parsing for protocol v2 success.

### Requirement: Claude host runner setup is explicit
The system SHALL make owned host-runner use explicit in configuration, previews, provider checks, and documentation.

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
- **THEN** Claude task previews and provider diagnostics identify that Claude will use the owned interactive host runner.

#### Scenario: Configured host runner is unavailable
- **WHEN** `AGENT_BRIDGE_CLAUDE_HOST_SOCKET` is configured but the socket cannot be reached or the runner reports a protocol mismatch
- **THEN** Claude task execution and smoke checks fail with an actionable host-runner diagnostic and do not silently fall back to sandboxed Claude execution, upstream `claude-p`, or native `claude -p`.

#### Scenario: Host runner is not configured
- **WHEN** `AGENT_BRIDGE_CLAUDE_HOST_SOCKET` is not configured
- **THEN** normal production Claude provider readiness is not launchable.
- **AND** diagnostics recommend configuring the owned Claude host runner rather than falling back to direct print-mode execution.

#### Scenario: Host runner and hook relay are distinct
- **WHEN** the owned interactive runner creates hook relay IPC
- **THEN** that runner-internal event log is distinct from the MCP-to-host-runner Unix socket.
- **AND** the host-runner socket never accepts hook payloads directly.

### Requirement: Claude host runner cleans up PTY children
The system SHALL terminate and reap PTY-driven Claude child processes reliably on timeout, disconnect, and runner shutdown.

#### Scenario: PTY task times out
- **WHEN** an owned interactive Claude task exceeds its timeout
- **THEN** the host runner closes the PTY master, sends termination to the Claude session or process group where supported, waits a bounded grace period, escalates if needed, and reaps the child.

#### Scenario: PTY client disconnects
- **WHEN** the MCP client disconnects while a PTY-driven Claude task is running
- **THEN** the host runner follows the same bounded PTY child cleanup sequence before reporting the disconnect outcome.
