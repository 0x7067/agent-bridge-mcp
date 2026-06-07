# Agent Bridge MCP

**Generated:** 2026-06-07T12:00:00Z
**Commit:** latest

Rust stdio MCP server that delegates bounded tasks to local provider agents (Claude Code, Codex, Cursor, Kimi/Pi, Antigravity). The caller retains responsibility for verification.

## STRUCTURE

```
.
├── crates/
│   └── agent-bridge-mcp/
│       ├── src/                 # (AGENTS.md)
│       │   ├── bin/
│       │   ├── claude_interactive/   # (AGENTS.md)
│       │   ├── server/
│       │   └── task/
│       └── tests/               # Integration tests + fixtures
├── docs/agents/                  # Architecture, guardrails, tooling, definition-of-done
├── scripts/quality.sh             # Local validation mirror
└── .github/workflows/
    ├── quality.yml                # Hard gates + informational reporting
    └── release-rust.yml           # Cross-platform binaries
```

## WHERE TO LOOK

| Task | Location |
|------|----------|
| Add/modify an MCP tool | `src/tools.rs` schema, `src/server.rs` dispatch |
| Spawn/observe/stop a task | `src/task.rs` (~110 kB — heavy), `src/task/supervision.rs` |
| Define a provider capability | `src/provider.rs` |
| Interactive PTY logic | `src/claude_interactive/` (AGENTS.md) |
| Host-runner integration | `src/claude_host.rs` |
| Protocol framing | `src/mcp.rs` |
| Diagnostic/reporting text | `src/guidance.rs` (~26 kB) |
| Binary entry | `src/main.rs` (delegates to `runtime.rs`) |

## CONVENTIONS

- Rust 2024 edition, resolver `"3"`, single-member workspace.
- `serde_json` uses `preserve_order` — keep deterministic field ordering in tool schemas and responses.
- **Zero tolerance CI**: `clippy -D warnings`, `cargo machete`, `jscpd` <5%. Pre-existing failures are not exempt.
- New deps must be deliberate; `cargo machete` fails on unused ones.
- Extend the tool surface via options, not new tools. Current surface is intentionally capped at eight.

## ANTI-PATTERNS

- Print to stdout in the server loop — stdout is the MCP JSON-RPC transport channel. Log to stderr only.
- Trust provider/subagent output as proof — always verify locally before marking done.
- Assume parallel PTY tests are safe — they touch global process state and can cross-flake.
- Add dependencies without checking `cargo machete` impact.

## COMMANDS

```bash
scripts/quality.sh                     # Run all hard gates locally
cargo test -- --test-threads=1          # Isolate flaky PTY tests
cargo run --bin agent-bridge-mcp        # Start the stdio server
```

## KEY CONFIGS

| Tool | Entry | Notes |
|------|-------|-------|
| Cargo | `Cargo.toml` (workspace) + `crates/*/Cargo.toml` | Edition 2024, publish=false |
| CI hard gates | `.github/workflows/quality.yml` | Fails on fmt, clippy warning, machete, jscpd ≥5% |
| Release builds | `.github/workflows/release-rust.yml` | Linux x64, macOS x64/arm64 |

## UNIQUE STYLES

- `AGENT_BRIDGE_FORCE_PANIC=1` forces a panic in `main_entry()` for integration-testing the panic hook.
- Second binary `agent-bridge-mcp-rs` lives in `src/bin/agent-bridge-mcp-rs.rs`; rarely touched.
- Panic hook attempts SIGTERM termination of all tracked provider children to prevent orphans.
- No TODO/FIXME/HACK/XXX markers in the codebase — maintain that bar.

## NOTES

- Shutdown handles SIGTERM (exit 143) and Ctrl-C (exit 130).
- `claude-host-runner` subcommand bypasses the MCP loop and binds to a Unix socket directly.
- The server writes newline-delimited JSON to stdout; blank input lines are ignored.
