# Definition of Done

A change is done only when every gate below passes with zero errors. Run them
locally before committing — `scripts/quality.sh` mirrors CI (`.github/workflows/quality.yml`).

## At a Glance

```bash
scripts/quality.sh
```

Exits non-zero if any **hard gate** fails. Reporting-only sections never fail.

## Hard Gates (Must Pass)

| Gate | Command | Threshold |
| --- | --- | --- |
| Format | `cargo fmt --all --check` | no diffs |
| Lints | `cargo clippy --all-targets -- -D warnings` | zero warnings |
| Unused deps | `cargo machete` | none |
| Duplication | `npx --yes jscpd` | < 5% (min 50 tokens / 10 lines) |

Pre-existing failures are NOT exempt — fix them as part of your change.

## Tests

Integration tests live in `crates/agent-bridge-mcp/tests/` with fixtures under
`tests/fixtures/`.

```bash
cargo test -p agent-bridge-mcp -- --test-threads=1
cargo test -p agent-bridge-mcp --test mcp_adapter_protocol
cargo test -p agent-bridge-mcp --test stdio_binary -- --test-threads=1
```

Notes:
- Some tests drive PTYs and global process state and can flake under parallel
  load — see `docs/agents/guardrails.md`. If you see cross-test flakiness, run
  `cargo test -- --test-threads=1`.
- cargo runs may pass through a buffering wrapper; redirect output to a file or
  use `--test-threads=1` if piped output looks truncated.

## Reporting-Only (Review, Don't Gate)

- Complexity hotspots via clippy `cognitive_complexity` / `too_many_lines` / `too_many_arguments`.
- Module dependency graph / acyclicity via `cargo modules` (flags benign self-refs).

## Going Deeper

- [Guardrails](guardrails.md) — PTY test hazards, MCP protocol contract, secrets
- [Testing workflows](../WORKFLOWS/unit-tests.md) — patterns for fake-provider tests, PTY tests, protocol tests
- [Quality workflow](../WORKFLOWS/backend.md) — running quality gates, interpreting output
