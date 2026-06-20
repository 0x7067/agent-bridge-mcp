# Guardrails

## At a Glance

- **Never print to stdout** — it's the MCP protocol channel. Use stderr.
- **Keep the public surface small** — ACP router by default; MCP adapter exposes only `agent_delegate` and `agent_evidence`.
- **Provider output is evidence, not proof** — verify locally before claiming done.
- **No secrets anywhere** — code, fixtures, commits, logs must stay clean.
- **PTY tests run single-threaded** — global process state flakes under parallelism.

## PTY & Global Process State in Tests

The interactive provider (`src/claude_interactive/`) spawns real child processes
through PTYs. Tests exercising it touch **global process state**, so:

- Parallel runs can flake — a process-group signal in one test can cross-kill
  another test's child. If you see nondeterministic failures, run with
  `cargo test -- --test-threads=1` to confirm before chasing a "real" bug.
- When adding PTY/interactive tests, scope signals and child kills to the
  specific child PID/PGID you own; never broadcast to a shared group.
- Don't assume timing — drive on observed transcript events, not sleeps.

## MCP Protocol Contract

- Transport is JSON-RPC over stdio. **stdout is the protocol channel** — never
  print debug output to stdout; use stderr/logging. A stray println corrupts the framing.
- `serde_json` is configured with `preserve_order`; keep field ordering stable in
  tool schemas and responses where callers or fixtures depend on it.
- Keep the public surface small (see `architecture.md`). ACP hosts use the router
  directly; MCP hosts use `agent_delegate` and `agent_evidence`.

## Evidence vs. Proof

Provider/subagent output is **evidence only**. The calling agent owns project
verification — run `scripts/quality.sh` and the relevant tests before reporting
a delegated task as done. This is the server's whole reason for existing; honor it
in our own work too.

## Secrets

No secrets, `.env` contents, auth tokens, or proprietary content in code, fixtures,
commits, or logs. Provider config examples (e.g. `codex-mcp.example.json`) must stay
example-only with placeholder values.

## Going Deeper

- [Security](security.md) — threat model, workspace confinement, secret hygiene
- [Testing workflows](../WORKFLOWS/unit-tests.md) — fake scripts, isolated PID registries, protocol tests
- [Backend workflows](../WORKFLOWS/backend.md) — modifying tools, adding providers, running gates

