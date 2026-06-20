# Deployment Guide

**Last verified:** 2026-06-20
**Deployment method:** CI/CD pipeline (GitHub Actions) for release binaries; manual distribution for ACP/MCP adapter client configs.

## Environments

This project ships as a **standalone binary**, not a hosted service.
"Environments" refer to the machine where Agent Bridge runs alongside the
consuming agent client.

| Environment | Delivery Method | Trigger | Approval Required |
|-------------|-----------------|---------|-------------------|
| Production  | GitHub Release artifact | Tag push (`v*`) | No (automated) |
| Local/dev   | `cargo build --release` | Developer-initiated | N/A |

## Pre-Deployment Checklist

- [ ] All tests passing on the branch
- [ ] `cargo clippy --all-targets -- -D warnings` is clean
- [ ] `cargo fmt --all --check` passes
- [ ] `cargo machete` finds no unused dependencies
- [ ] `npx --yes jscpd` duplication ≤ 5%
- [ ] Version bumped in `Cargo.toml` if releasing

## Deployment Process

### Release Builds

**Trigger:** Push a Git tag matching `v*` (e.g., `git tag v0.2.0 && git push origin v0.2.0`).

**Pipeline steps:**

| Step | What Happens | Duration | Failure Action |
|------|-------------|----------|--------------|
| 1 | Checkout code | ~10s | Pipeline stops |
| 2 | Install Rust target | ~30s | Pipeline stops |
| 3 | Build release binary (`cargo build --locked --release --target <triple> --bin agent-bridge-mcp`) | ~2–5min | Pipeline stops |
| 4 | Package artifact + compute SHA256 | ~10s | Pipeline stops |
| 5 | Upload artifact | ~10s | Pipeline stops |

Runs in parallel for three targets:
- `ubuntu-latest` → `x86_64-unknown-linux-gnu`
- `macos-13` → `x86_64-apple-darwin`
- `macos-14` → `aarch64-apple-darwin`

**CI/CD configuration file:** `.github/workflows/release-rust.yml`

### Manual Distribution

Download the appropriate artifact from the GitHub Actions run (or attach to a
Release), place it on PATH, and wire it into either your ACP client launch
configuration or your MCP client JSON config.

Example MCP adapter client snippet:

```json
{
  "mcpServers": {
    "agent-bridge": {
      "command": "/usr/local/bin/agent-bridge-mcp",
      "args": ["mcp-adapter"],
      "env": {
        "AGENT_BRIDGE_WORKSPACES": "/home/user/projects",
        "AGENT_BRIDGE_STATE_DIR": "~/.agent-bridge-mcp/state"
      }
    }
  }
}
```

## Post-Deployment Verification

| Check | Command or Step | Expected Result |
|-------|-----------------|-----------------|
| Binary executes | `agent-bridge-mcp --version` or an ACP `initialize` request | Responds with version/server metadata |
| Provider smoke | `agent-bridge-mcp --doctor-smoke --provider codex` | Reports provider readiness |
| Registry state | `ls ~/.agent-bridge-mcp/state` | Directory readable/writeable |

## Rollback Procedure

### When to Rollback

- Binary panics on initialization
- ACP or MCP-adapter handshake fails
- Regression in delegated task behavior

### Steps

1. Replace the binary on PATH with the previous known-good artifact.
2. Restart the consuming client (most MCP hosts restart the subprocess on reconnect).
3. Verify with the Post-Deployment Verification checks above.

### No Database Migrations

This project does not use a relational database. State is stored as JSON files in the filesystem. If a new version introduces incompatible registry layout, clear or backup `AGENT_BRIDGE_STATE_DIR` and let the new version recreate it.

**WARNING:** Clearing state loses pending/running task metadata. Inspect and drain tasks first.

## Troubleshooting

| Symptom | Likely Cause | Resolution |
|---------|-------------|------------|
| `cargo build --release` fails | Missing C toolchain | Install Clang/Xcode/GCC |
| Release artifact won’t start | Wrong target triple | Download the artifact matching your CPU/OS |
| Client disconnects instantly | Binary crashes on startup | Run binary directly in terminal to see stderr |
| Claude provider “host runner” not ready | Socket not running or wrong path | Start `claude-host-runner` subcommand and verify socket path |
| Duplicate code gate fails | Refactoring drift | Review `jscpd` report; refactor shared helpers |
