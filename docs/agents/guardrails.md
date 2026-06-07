# Guardrails

## PTY & global process state in tests

The interactive provider (`src/claude_interactive/`) spawns real child processes
through PTYs. Tests exercising it touch **global process state**, so:

- Parallel runs can flake — a process-group signal in one test can cross-kill
  another test's child. If you see nondeterministic failures, run with
  `cargo test -- --test-threads=1` to confirm before chasing a "real" bug.
- When adding PTY/interactive tests, scope signals and child kills to the
  specific child PID/PGID you own; never broadcast to a shared group.
- Don't assume timing — drive on observed transcript events, not sleeps.

## MCP protocol contract

- Transport is JSON-RPC over stdio. **stdout is the protocol channel** — never
  print debug output to stdout; use stderr/logging. A stray println corrupts the framing.
- `serde_json` is configured with `preserve_order`; keep field ordering stable in
  tool schemas and responses where callers or fixtures depend on it.
- Keep the tool surface at eight tools (see `architecture.md`). Extend via options,
  not new tools, unless there's a strong reason.

## Evidence vs. proof

Provider/subagent output is **evidence only**. The calling agent owns project
verification — run `scripts/quality.sh` and the relevant tests before reporting
a delegated task as done. This is the server's whole reason for existing; honor it
in our own work too.

## Secrets

No secrets, `.env` contents, auth tokens, or proprietary content in code, fixtures,
commits, or logs. Provider config examples (e.g. `codex-mcp.example.json`) must stay
example-only with placeholder values.
