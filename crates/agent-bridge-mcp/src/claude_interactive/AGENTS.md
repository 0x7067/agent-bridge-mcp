# Interactive PTY Provider

**Generated:** 2026-06-07T12:00:00Z

Spawns Claude Code (or similar) in a pseudo-terminal, drives it via ANSI byte streams, parses transcripts, detects setup prompts, and classifies failures.

## STRUCTURE

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

## WHERE TO LOOK

| Task | Location |
|------|----------|
| Spawn or resize a PTY | `pty.rs` |
| Drive interaction (send keys, await probes) | `runner.rs` |
| Parse transcript events from raw bytes | `transcript.rs` |
| Detect sandbox/approval/setup prompts | `setup.rs` |
| Classify abnormal stops | `failure.rs` |
| Relay sidecar hooks | `hooks.rs` |

## ANTI-PATTERNS

- Broadcast signals to process groups in tests — scoped PID targeting only.
- Use sleep-based synchronization — rely on observable transcript events.
- Ignore ANSI escape sequences — always strip before parsing prompts.
- Leave orphan processes after panics — the runtime panic hook SIGTERMs active PIDs, but local cleanup still matters.
