---
status: accepted
date: 2025-02-??
---

# ADR-0005: Route Claude Through an Owned PTY Host Runner via Unix Socket

## Context

Early Claude provider attempts invoked `claude` directly via `fork/exec` with the prompt passed on stdin. This broke because:

- Claude Code expects a TTY for interactive mode (authentication prompts, editor launches, confirmations).
- Sandboxed MCP server environments (e.g., some containerized hosts) lack TTY allocation.
- macOS Keychain-backed Claude auth requires a non-sandboxed shell context.
- Passing sensitive prompt text through process `argv` risks exposure in `ps` listings.

A solution was needed that kept the MCP server a stdio process while giving Claude a proper interactive environment.

## Decision

We decided to introduce a sidecar `claude-host-runner` subcommand that:

1. Opens a Unix-domain socket listener.
2. Accepts a framed protobuf-like JSON request from the MCP server.
3. Spawns Claude Code inside a PTY owned by the host runner.
4. Injects prompts via PTY keystrokes (not `argv`).
5. Captures the full ANSI transcript, stop events, and failures.
6. Returns structured results over the socket.

The MCP server connects to this socket via `AGENT_BRIDGE_CLAUDE_HOST_SOCKET`. The host runner must be started outside restricted sandboxes, typically in a trusted user shell.

### Considered Alternatives

#### Direct PTY in the MCP server process

- Good, because simpler architecture (one process).
- Bad, because the MCP server itself must remain a clean stdio process; attaching a PTY corrupts the JSON-RPC transport.

#### SSH remote execution

- Good, because full environment flexibility.
- Bad, because adds networking/crypto dependency and latency for a local-only tool.

## Consequences

### Positive

- Claude Code runs in its intended interactive environment.
- Prompt text never appears in `ps` or shell history.
- Host runner can hold Keychain credentials the MCP server cannot access.

### Negative

- Extra operational step: user must start the host runner before using Claude provider.
- Socket path coordination between runner and MCP server is a configuration footgun.
- PTY emulation complexity (ANSI parsing, terminal probes, setup prompt detection) is significant.

### Neutral

- Only Claude uses this path; other providers remain direct fork/exec.

## Evidence

- **Commit(s):** `d4af8a5`, `ee41239`, `a7625b6`, `56ceeb5`, `aef96c2`
- **Key files changed:** `src/claude_host.rs`, `src/claude_interactive/runner.rs`, `src/claude_interactive/pty.rs`, `src/claude_interactive/hooks.rs`, `src/claude_interactive/setup.rs`, `src/claude_interactive/transcript.rs`, `src/claude_interactive/terminal.rs`, `src/provider.rs`
- **Blast radius:** 8+ files spanning PTY, socket, protocol, and provider adaptation.
- **Timeline:** Series of incremental PRs over ~2–3 weeks.
