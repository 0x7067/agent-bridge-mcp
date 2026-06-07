# Interactive PTY Provider

**Generated:** 2026-06-07T12:00:00Z

Spawns Claude Code (or similar) in a pseudo-terminal, drives it via ANSI byte streams, parses transcripts, detects setup prompts, and classifies failures.

## At a Glance

- Owns the only PTY in the codebase; all other providers are vanilla fork/exec.
- Communicates with the main server via a Unix socket and a custom JSON protocol (v2).
- Prompts are injected as keystrokes, never command-line arguments — keeps secrets out of `ps`.
- The ANSI stream is parsed into structured events; setup prompts (login, workspace trust) are intercepted.

## Structure

```
claude_interactive/
├── pty.rs          # PtySpawn / PtySession abstractions
├── terminal.rs       # TerminalProbeHandler / ProbeChunk parsing
├── runner.rs         # Orchestration: spawn → interact → collect
├── transcript.rs     # Event parsing + validation
├── hooks.rs          # HookRelay / HookSettings relay
├── setup.rs          # Prompt detection + ANSI stripping
└── failure.rs        # Failure categorization (sandbox rejection, stalls, etc.)
```

## Where to Look

| Task | Location |
|------|----------|
| Spawn or resize a PTY | `pty.rs` |
| Drive interaction (send keys, await probes) | `runner.rs` |
| Parse transcript events from raw bytes | `transcript.rs` |
| Detect sandbox/approval/setup prompts | `setup.rs` |
| Classify abnormal stops | `failure.rs` |
| Relay sidecar hooks | `hooks.rs` |

## Anti-Patterns

- Broadcast signals to process groups in tests — scoped PID targeting only.
- Use sleep-based synchronization — rely on observable transcript events.
- Ignore ANSI escape sequences — always strip before parsing prompts.
- Leave orphan processes after panics — the runtime panic hook SIGTERMs active PIDs, but local cleanup still matters.
- Leak PTY implementation details into `server.rs` or `task.rs` — this boundary is opaque to the rest of the crate.

## Going Deeper

- [Docs/agents guardrails](../../../../docs/agents/guardrails.md) — PTY test hazards, global process state
- [Integrations codemap](../../../../CODEMAPS/integrations.md) — host protocol, data flow, external dependencies
- [ADR-0005](../../../../ADR/0005-claude-pty-host-runner.md) — why the PTY host runner exists and why it uses a socket
- [Security](../../../../SECURITY.md) — threat model notes on PTY isolation and prompt injection

