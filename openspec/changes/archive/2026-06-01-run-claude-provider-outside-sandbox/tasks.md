## 1. Protocol And Launch Model

- [x] 1.1 Add failing tests for Claude host-runner preview and smoke diagnostics, including launch strategy reporting and unavailable-runner failure.
- [x] 1.2 Add failing tests for host-runner request validation: structured Claude-p requests only, command descriptor rejection, oversized/unterminated request rejection, protocol version mismatch, ping, workspace canonicalization failure, workspace policy id mismatch, symlink escape rejection, socket symlink/file/workspace-path rejection, live-vs-stale socket handling, timeout bounds, bounded streaming output caps, sanitized errors, and sanitized runner logs.
- [x] 1.3 Add failing tests for runner cleanup on client EOF during execution and SIGTERM/SIGINT graceful shutdown.
- [x] 1.4 Add failing tests for deterministic workspace policy id computation from sorted canonical workspace paths.
- [x] 1.5 Extend provider command descriptors with structured Claude host-runner launch metadata while preserving direct execution when no host socket is configured.

## 2. Host Runner Implementation

- [x] 2.1 Implement a bridge-owned Claude host-runner protocol over a local Unix socket with newline-delimited JSON framing and request/response versioning.
- [x] 2.2 Implement runner-side `claude-p` command reconstruction from structured request fields; do not accept executable paths, arbitrary argv, or shell command descriptors over the protocol.
- [x] 2.3 Implement the host-runner executable mode that requires canonicalizable `AGENT_BRIDGE_WORKSPACES`, verifies `claude-p` is executable before binding, validates owner-only socket directory permissions, validates unsafe socket paths, handles stale sockets without unlinking live sockets, caps request-line reads before JSON parsing, validates requests before spawning `claude-p`, captures stdout/stderr with bounded streaming caps, enforces Unix process-group timeouts where supported, monitors client disconnects for cleanup, handles SIGTERM/SIGINT cleanup, emits sanitized logs only, and returns structured results.
- [x] 2.4 Implement protocol ping and stable error responses with discriminated error codes.
- [x] 2.5 Route Claude provider smoke checks through the host runner when `AGENT_BRIDGE_CLAUDE_HOST_SOCKET` is configured.
- [x] 2.6 Route Claude task execution through the host runner when configured, preserving task logs, result parsing, diagnostics, redaction, and lifecycle behavior.

## 3. Documentation And Configuration

- [x] 3.1 Update README setup and troubleshooting docs for macOS Keychain-backed Claude auth, `AGENT_BRIDGE_CLAUDE_HOST_SOCKET`, host-runner lifecycle, and restarting the runner after workspace configuration changes.
- [x] 3.2 Update Codex MCP configuration guidance to include the host socket environment variable without exposing secrets.
- [x] 3.3 Add or update deterministic fake-provider tests so CI does not require live Claude auth, Keychain access, or model quota.

## 4. Verification

- [x] 4.1 Run targeted host-runner and Claude provider tests and confirm they pass.
- [x] 4.2 Run `rtk openspec validate run-claude-provider-outside-sandbox`.
- [x] 4.3 Build the Rust MCP binary.
- [x] 4.4 Start the host runner outside the sandbox, verify the runner socket, and run a Claude-only live smoke with real `claude-p`, confirming host-runner launch strategy and successful provider output.
