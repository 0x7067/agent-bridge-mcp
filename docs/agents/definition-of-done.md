# Definition of Done

A change is done only when every gate below passes with zero errors. Run them
locally before committing — `scripts/quality.sh` mirrors CI (`.github/workflows/quality.yml`).

## One command

```bash
scripts/quality.sh
```

It exits non-zero if any **hard gate** fails. Complexity and dependency-graph
sections are reporting-only and never fail.

## Hard gates (must pass)

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
cargo test                          # full suite
cargo test --test server_protocol   # one integration test target
```

Notes:
- Some tests drive PTYs and global process state and can flake under parallel
  load — see `docs/agents/guardrails.md`. If you see cross-test flakiness, run
  `cargo test -- --test-threads=1`.
- cargo runs may pass through a buffering wrapper; redirect output to a file or
  use `--test-threads=1` if piped output looks truncated.

## Reporting-only (review, don't gate)

- Complexity hotspots via clippy `cognitive_complexity` / `too_many_lines` / `too_many_arguments`.
- Module dependency graph / acyclicity via `cargo modules` (flags benign self-refs).
