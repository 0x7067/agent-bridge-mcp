# Agent Bridge MCP

Agent Bridge MCP is a Rust stdio MCP server for delegating bounded work from one
agent client to local provider agents such as Claude Code, Codex, Cursor Agent,
Kimi/Pi, and Google Antigravity CLI.

It exposes a provider-neutral lifecycle so a caller can preview, start, observe,
inspect, stop, and remove delegated tasks while keeping the main agent
responsible for verification.

## What It Provides

- Provider discovery and readiness checks through `providers_list` and
  `providers_check`.
- Setup diagnostics through `doctor`, including workspace policy, provider,
  client config, Claude host-runner, and binary freshness checks.
- Primary task launch and lifecycle tools:
  - `agent_spawn`
  - `agent_observe`
  - `agent_result`
  - `agent_remove`
- Focused diagnostic, presentation, and control tools:
  - `agent_preview`
  - `agent_list`
  - `agent_status`
  - `agent_wait`
  - `agent_logs`
  - `agent_transcript`
  - `agent_stop`
- MCP self-description through prompts and guidance resources.
- Deterministic fake-provider tests that do not require paid model access,
  network access, provider credentials, or host keychain permissions.

## Providers

First-class provider adapters:

| Provider | Local CLI | Notes |
| --- | --- | --- |
| `claude` | `claude` | Runs through the Agent Bridge-owned interactive PTY host runner. |
| `codex` | `codex exec` | Uses noninteractive Codex execution. |
| `cursor` | `cursor-agent -p` | Uses Cursor Agent prompt mode. |
| `kimi` | `pi -p` | Uses the local Pi/Kimi CLI. |
| `antigravity` | `agy --print` | Uses Antigravity CLI print mode. |

Supported task modes are `research`, `review`, `implement`, and `command`.
Provider/mode combinations are validated before launch.

## Requirements

- Rust toolchain with Cargo.
- `git` on `PATH`.
- Optional provider CLIs depending on which providers you want to use:
  `claude`, `codex`, `cursor-agent`, `pi`, and/or `agy`.
- For Claude provider tasks: run the Claude host runner outside restricted
  sandboxes and point the MCP server at its Unix socket with
  `AGENT_BRIDGE_CLAUDE_HOST_SOCKET`.

## Build And Test

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release --bin agent-bridge-mcp
```

The default test suite uses fake providers and local fixtures.

## Install

Build the release binary and place it somewhere on your `PATH`:

```bash
cargo build --release --bin agent-bridge-mcp
install -m 0755 target/release/agent-bridge-mcp ~/.local/bin/agent-bridge-mcp
```

Run the MCP server over stdio:

```bash
agent-bridge-mcp
```

## MCP Client Configuration

Example MCP config:

```json
{
  "mcpServers": {
    "agent-bridge": {
      "command": "agent-bridge-mcp",
      "args": [],
      "env": {
        "AGENT_BRIDGE_WORKSPACES": "/path/to/workspaces",
        "AGENT_BRIDGE_STATE_DIR": "~/.agent-bridge-mcp/state"
      }
    }
  }
}
```

`AGENT_BRIDGE_WORKSPACES` is a platform path-list of allowed workspace roots.
Task `cwd` values must resolve inside one of those roots.

`AGENT_BRIDGE_STATE_DIR` is optional. When omitted, Agent Bridge stores state in
`~/.agent-bridge-mcp/state`.

## Claude Host Runner

Claude tasks use an Agent Bridge-owned host runner so interactive Claude Code can
run in a PTY while the MCP server stays a stdio process.

Start the host runner in a trusted shell:

```bash
mkdir -p ~/.agent-bridge-mcp/run
AGENT_BRIDGE_WORKSPACES="/path/to/workspaces" \
  agent-bridge-mcp claude-host-runner ~/.agent-bridge-mcp/run/claude-host.sock
```

Expose the same socket to the MCP server:

```json
{
  "mcpServers": {
    "agent-bridge": {
      "command": "agent-bridge-mcp",
      "env": {
        "AGENT_BRIDGE_WORKSPACES": "/path/to/workspaces",
        "AGENT_BRIDGE_STATE_DIR": "~/.agent-bridge-mcp/state",
        "AGENT_BRIDGE_CLAUDE_HOST_SOCKET": "~/.agent-bridge-mcp/run/claude-host.sock"
      }
    }
  }
}
```

Run `doctor` and then `providers_check` with `smoke: true` when validating a
Claude setup.

## Recommended Workflow

1. Call `doctor` when setup or provider readiness is uncertain.
2. Call `providers_check` only for focused readiness follow-up; use
   `smoke: true` when launch readiness matters.
3. Call `agent_spawn` to start a bounded provider task.
4. Use `agent_observe` to monitor progress and follow `nextActions`.
5. Use `agent_result` after finalization to inspect logs, transcript metadata,
   changed files, diff, diagnostics, and the derived review packet.
6. Run local project verification yourself before trusting delegated output.
7. Call `agent_remove` intentionally after inspecting any managed worktree.

Diagnostic tools stay available when the primary path is not enough:
`agent_preview` inspects launch construction, `agent_list` and `agent_status`
support native presentation and state reads, `agent_wait` handles simple
finality waits, `agent_logs` and `agent_transcript` expose raw evidence, and
`agent_stop` terminates agents that are no longer useful.

Provider output is evidence, not proof. The caller remains responsible for tests,
lint, build, review, and cleanup.

## Breaking API Simplification

The public lifecycle surface is agent-oriented only:

- Use `agent_*` tools; public `task_*` lifecycle tools are not supported.
- Use `agentId` for follow-up lifecycle calls and response parsing.
- `taskId` is rejected as an unknown public argument.
- New lifecycle IDs use the `agent_...` prefix.
- Existing registries written with old `taskId` records are not migrated; use a
  fresh state directory if you need to discard old records.

## Safety Model

- Task cwd values are confined to configured workspace roots.
- Prompts are bounded and redacted from diagnostics where possible.
- Public tool arguments reject unknown fields.
- Provider output, git status, git diff, stdout, and stderr are capped.
- Most providers receive a restricted environment allowlist.
- Claude provider prompt text is injected through PTY input, not process argv.
- Antigravity research/review tasks pass `--sandbox`, but Agent Bridge treats
  non-mutating behavior as prompt-enforced unless the local Antigravity sandbox
  has been separately verified.
- Managed worktrees are preserved for inspection until explicitly removed.

## Release Artifacts

`.github/workflows/release-rust.yml` builds release artifacts for:

- Linux x64
- macOS x64
- macOS arm64

## License

MIT. See [LICENSE](LICENSE).
