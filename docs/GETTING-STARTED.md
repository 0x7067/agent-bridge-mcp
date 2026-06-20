# Getting Started

**Last verified:** 2026-06-20

## Prerequisites

Install these before proceeding. Version numbers are minimum requirements.

| Tool       | Version | Install Command                                  | Verify Command          |
| ---------- | ------- | ------------------------------------------------ | ----------------------- |
| Rust       | Latest stable | [rustup.rs](https://rustup.rs/)             | `rustc --version`       |
| Cargo      | Bundled with Rust | Comes with Rust                           | `cargo --version`       |
| git        | 2.x+    | Platform package manager                         | `git --version`         |

Optional (only if using the corresponding provider):

| Tool        | Install Command                                        | Verify Command      |
| ----------- | ------------------------------------------------------ | ------------------- |
| claude      | Follow Anthropic instructions                          | `claude --version`  |
| codex       | `npm install -g @openai/codex`                         | `codex --version`   |
| cursor-agent | Download from Cursor                                    | `cursor-agent --version` |
| pi          | Follow Kimi/Pi installation instructions                 | `pi --version`      |
| forge       | Follow Forge installation instructions                   | `forge --version`   |
| agy         | Follow Antigravity CLI installation instructions        | `agy --version`     |

## Clone and Install

```bash
git clone https://github.com/0x7067/agent-bridge-mcp.git
cd agent-bridge-mcp
cargo build --release --bin agent-bridge-mcp
```

## Configure Environment

No `.env.example` found — environment variables are optional for basic operation. Refer to `docs/SETUP.md` for full variable descriptions.

Minimal recommended environment:

| Variable                  | Example Value                      | Description                                      |
|---------------------------|-------------------------------------|--------------------------------------------------|
| `AGENT_BRIDGE_WORKSPACES` | `/home/user/projects:/opt/repos`   | Allowed workspace roots for delegated tasks      |
| `AGENT_BRIDGE_STATE_DIR`  | `~/.agent-bridge-mcp/state`        | Persistent state directory (auto-created)        |

## Start Agent Bridge

```bash
./target/release/agent-bridge-mcp
```

The default process runs the ACP router. It reads newline-delimited JSON-RPC
requests from **stdin** and writes responses to **stdout**. Blank lines are
ignored.

There is no web UI. Use an ACP-capable client directly, or run
`agent-bridge-mcp mcp-adapter` for hosts that require MCP tools.

## Run Tests

```bash
cargo test -- --test-threads=1
```

Some tests exercise the PTY subsystem and spawn child processes; they may flake under parallelism. Use `--test-threads=1` to isolate them.

## Your First PR

1. Create a branch: `git checkout -b feat/short-description`
2. Make changes
3. Run quality checks: `./scripts/quality.sh`
4. Commit using conventional commits: `feat(scope): description`
5. Push and open a PR against `main`
