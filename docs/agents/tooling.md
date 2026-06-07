# Tooling & Build

## At a Glance

- Single crate workspace, Rust 2024 edition, resolver `"3"`.
- Two binaries: `agent-bridge-mcp` (primary) and `agent-bridge-mcp-rs` (alternate).
- Four hard CI gates: rustfmt, clippy `-D warnings`, cargo-machete, jscpd <5%.
- One-shot install: `cargo install cargo-modules cargo-machete --locked`.

## Workspace

Cargo workspace, `resolver = "3"`, single member `crates/agent-bridge-mcp`
(Rust 2024 edition). All cargo commands run from the repo root.

## Binaries

- `agent-bridge-mcp` — `src/main.rs`, the primary stdio MCP server.
- `agent-bridge-mcp-rs` — `src/bin/agent-bridge-mcp-rs.rs`, alternate entrypoint.

```bash
cargo build                       # debug build, both binaries
cargo run --bin agent-bridge-mcp  # run the stdio server
```

## Dependencies

- Add deps deliberately; `cargo machete` (a hard CI gate) fails on unused crates.
- Prefer existing deps before adding new ones. Tokio features are explicit in
  `Cargo.toml` — add a feature there if you need a new part of the runtime.
- Key crates: `tokio` (async), `pty-process` (interactive provider PTYs),
  `serde`/`serde_json` with `preserve_order`, `chrono`, `uuid`, `libc`.

## Formatting & Lints

- `cargo fmt --all` before committing; CI runs `cargo fmt --all --check`.
- `cargo clippy --all-targets -- -D warnings` — warnings are errors.
- Complexity lints (`cognitive_complexity`, `too_many_lines`, `too_many_arguments`)
  run informationally; recent history shows active refactoring to keep functions small.

## One-Time Tool Install

```bash
cargo install cargo-modules cargo-machete --locked
# jscpd is fetched on demand via npx (Node >= 18)
```

## Going Deeper

- [Getting started](getting-started.md) — clone, build, run, first PR
- [Definition of Done](definition-of-done.md) — exact gate commands and thresholds
- [Setup](../SETUP.md) — full environment setup, troubleshooting, Claude host-runner
- [Deployment](../DEPLOYMENT.md) — release builds, CI pipeline, rollback

