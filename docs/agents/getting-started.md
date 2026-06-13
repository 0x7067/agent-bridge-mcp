# Getting Started

Clone, build, and run the MCP server in under 15 minutes.

## At a Glance

- One Rust crate, two binaries (`agent-bridge-mcp`, `agent-bridge-mcp-rs`).
- No database, no docker, no cloud services — just a stdio binary + filesystem state.
- Tests run offline with fake provider scripts; no API keys required.

## Prerequisites

| Tool | Minimum | Verify |
|------|---------|--------|
| Rust | Latest stable | `rustc --version` |
| git | 2.x | `git --version` |

Optional provider CLIs for smoke testing: `claude`, `codex`, `cursor-agent`, `pi`, `forge`, `agy`.

## Clone & Build

```bash
git clone https://github.com/0x7067/agent-bridge-mcp.git
cd agent-bridge-mcp
cargo build --release --bin agent-bridge-mcp
```

## Run the Server

```bash
./target/release/agent-bridge-mcp
```

Expects newline-delimited JSON-RPC on stdin; writes responses to stdout. Blank lines are ignored.

Configure via env:

```bash
export AGENT_BRIDGE_WORKSPACES="$HOME/projects"
export AGENT_BRIDGE_STATE_DIR="$HOME/.agent-bridge-mcp/state"
```

## Run Tests

```bash
# Full suite (isolate PTY/process tests to single-threaded)
cargo test -- --test-threads=1

# Specific target
cargo test --test server_protocol
```

## First Contribution

```bash
# 1. Create branch
git checkout -b feat/short-description

# 2. Make changes

# 3. Validate (this is the hard-gate mirror of CI)
./scripts/quality.sh

# 4. Commit atomically
#    Format: feat(scope): description

# 5. Push and open PR against main
```

## Going Deeper

- [Full setup](../SETUP.md) — troubleshooting, env variables, Claude host-runner setup
- [Deployment](../DEPLOYMENT.md) — release builds, CI pipelines, rollback
- [Definition of Done](definition-of-done.md) — exact gates and thresholds
- [Tooling](tooling.md) — workspace structure, dependencies, one-time installs
