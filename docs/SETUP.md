# Full Environment Setup

**Last verified:** 2026-06-07
**Estimated time:** 15–25 minutes
**Target OS:** macOS / Linux (Windows untested)

## Step 1: System Prerequisites

### Runtime

| Requirement | Version | Why This Version |
| ----------- | ------- | ---------------- |
| Rust        | Latest stable via rustup | Required by edition 2024 and resolver `"3"` |
| Cargo       | Bundled with Rust | Workspace build orchestration |

No `.nvmrc` or `.node-version` file exists — Node is only needed for the `jscpd` duplication check in CI/local quality script.

### Package Manager

| Manager | Version | Install |
|---------|---------|---------|
| Cargo   | Bundled with Rust | Included in rustup |

### System Services

No host-local services (database, cache, queue) are required. The server is standalone.

| Service | Version | Install | Verify |
|---------|---------|---------|--------|
| git     | 2.x+    | Platform package manager | `git --version` |

## Step 2: Clone and Install Dependencies

```bash
git clone https://github.com/0x7067/agent-bridge-mcp.git
cd agent-bridge-mcp
cargo build --release --bin agent-bridge-mcp
```

**Troubleshooting:**

- If `cargo build` fails with a linker error: ensure your platform’s C compiler/linker is available (Clang/GCC on Linux/macOS).
- If native modules (PTY) fail to compile: install `pkg-config` and platform headers (Linux: `libssl-dev`, macOS: Xcode Command Line Tools).

## Step 3: Environment Configuration

No `.env.example` exists. Export variables directly or inject them via your MCP client configuration.

### Environment Variables Reference

| Variable                         | Required | Default                          | Description                                           |
|----------------------------------|----------|---------------------------------|-------------------------------------------------------|
| `AGENT_BRIDGE_WORKSPACES`        | Strongly recommended | none | Colon-separated list of allowed workspace roots |
| `AGENT_BRIDGE_STATE_DIR`         | No       | `~/.agent-bridge-mcp/state`      | Directory for persisted registry and task state       |
| `AGENT_BRIDGE_CLAUDE_HOST_SOCKET`| No       | none                             | Unix socket path for the Claude host runner           |
| `AGENT_BRIDGE_FORCE_PANIC`       | No       | unset                            | Set to `1` to force a panic in `main_entry()` (integration test only) |

#### Claude Host Runner Socket

If using the Claude provider, start the host runner separately:

```bash
mkdir -p ~/.agent-bridge-mcp/run
agent-bridge-mcp claude-host-runner ~/.agent-bridge-mcp/run/claude-host.sock
```

Then expose `AGENT_BRIDGE_CLAUDE_HOST_SOCKET` to the MCP server.

## Step 4: Local Services

None. There is no Docker Compose, no database daemon, and no external service mesh. The server is a single binary.

## Step 5: State Directory Setup

The state directory is automatically created on first use. To pre-create it:

```bash
mkdir -p ~/.agent-bridge-mcp/state
```

**Verify:**

```bash
ls ~/.agent-bridge-mcp/state
```

Should contain JSON registry files after the first delegated task.

## Step 6: Start the Application

```bash
./target/release/agent-bridge-mcp
```

**Verify the server is running:**

Send an MCP initialize request via stdin (or use an MCP client):

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0.0"}}}
```

Expected response: a JSON-RPC result containing `serverInfo.name` = `agent-bridge-mcp`.

## Step 7: Run the Test Suite

```bash
cargo test -- --test-threads=1
```

**Expected output:** ~20+ tests passing across protocol, binary, PTY, and fixture suites.

**If tests fail on a fresh setup:**

- Ensure `git` is on PATH (some tests rely on git commands).
- Run PTY-sensitive tests individually: `cargo test --test pty_adapter_spike -- --test-threads=1`
- Check that no stale `agent-bridge-mcp` processes are holding sockets or pseudo-terminals.

## Quick Reference Card

| Task                               | Command                                                    |
|------------------------------------|------------------------------------------------------------|
| Start dev server                   | `cargo run --bin agent-bridge-mcp`                         |
| Run all tests                      | `cargo test -- --test-threads=1`                           |
| Run specific test                  | `cargo test --test <name> -- --test-threads=1`             |
| Format check                       | `cargo fmt --all --check`                                  |
| Lint (zero warnings)               | `cargo clippy --all-targets -- -D warnings`                |
| Unused deps check                  | `cargo machete`                                            |
| Duplication check                  | `npx --yes jscpd`                                          |
| Quality gate (all hard checks)     | `./scripts/quality.sh`                                     |
| Build release binary               | `cargo build --release --bin agent-bridge-mcp`             |
| Build secondary binary             | `cargo build --release --bin agent-bridge-mcp-rs`          |
| Install locally                    | `install -m 0755 target/release/agent-bridge-mcp ~/.local/bin/agent-bridge-mcp` |
