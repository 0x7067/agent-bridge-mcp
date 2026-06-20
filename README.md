# Agent Bridge MCP

Agent Bridge MCP is a Rust stdio server that delegates bounded work from one
agent client to local provider agents such as Claude Code, Codex, Cursor Agent,
Kimi/Pi, and Google Antigravity CLI.

It runs an ACP router by default and exposes a small MCP adapter for clients
that cannot launch ACP agents directly. Provider output is evidence, not proof:
the caller remains responsible for local verification.

## What It Provides

- **ACP router** — the default `agent-bridge-mcp` invocation speaks
  newline-delimited ACP-flavored JSON-RPC.
- **MCP adapter** — `agent-bridge-mcp mcp-adapter` exposes two tools for
  non-ACP hosts.
- **Provider-neutral routing** — one `session/prompt` can fail over across
  configured provider candidates.
- **Bounded evidence** — `agent_evidence` reads transcript, stdout, stderr,
  diff, summary, or changed-files sections on demand.
- **Deterministic fake-provider tests** that do not require paid model access,
  network access, provider credentials, or host keychain permissions.

## Runtime Modes

`agent-bridge-mcp` runs the ACP router by default over newline-delimited JSON-RPC.

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
{"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"/repo"}}
{"jsonrpc":"2.0","id":3,"method":"session/prompt","params":{"sessionId":"router-...","mode":"implement","prompt":{"type":"text","text":"Fix the failing test"},"timeoutSeconds":600}}
```

Hosts that can only integrate external tools through MCP can run:

```bash
agent-bridge-mcp mcp-adapter
```

The adapter exposes two tools:

- `agent_delegate`: run one routed provider turn and return a terminal router result.
- `agent_evidence`: fetch bounded evidence for an evidence reference returned by `agent_delegate`.

The router result always includes `verificationStatus: "not_verified"`. Provider output is evidence; the caller remains responsible for local verification.

## Providers

First-class provider adapters:

| Provider | ACP command | Notes |
| --- | --- | --- |
| `claude` | `CLAUDE_ACP_BIN` or `claude-agent` | Launches through ACP. |
| `codex` | `CODEX_ACP_BIN` | Required ACP command. |
| `cursor` | `CURSOR_ACP_BIN` | Required ACP command. |
| `kimi` | `KIMI_ACP_BIN` or `kimi acp` | Launches through ACP. |
| `forge` | `FORGE_ACP_BIN` | Required ACP command. |
| `antigravity` | `ANTIGRAVITY_ACP_BIN` | Required ACP command. |

Supported task modes are `research`, `review`, `implement`, and `command`.
Provider/mode combinations are validated before launch.

## Requirements

- Rust toolchain with Cargo.
- `git` on `PATH`.
- Optional ACP-capable provider commands depending on which providers you want
  to use. Extra launch arguments can be supplied with the matching
  `*_ACP_ARGS` variable.
- The Claude host-runner diagnostic surface may still be configured with
  `AGENT_BRIDGE_CLAUDE_HOST_SOCKET`, but provider task launches use ACP.

## Build And Test

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release --bin agent-bridge-mcp
```

The default test suite uses fake providers and local fixtures. To smoke-test a
real local provider configuration, run:

```bash
agent-bridge-mcp --doctor-smoke --provider codex
```

## Install

Build the release binary and place it somewhere on your `PATH`:

```bash
cargo build --release --bin agent-bridge-mcp
install -m 0755 target/release/agent-bridge-mcp ~/.local/bin/agent-bridge-mcp
```

Run the ACP router over stdio:

```bash
agent-bridge-mcp
```

Check the installed binary and effective config without starting stdio:

```bash
agent-bridge-mcp --help
agent-bridge-mcp --version
agent-bridge-mcp --config-check
agent-bridge-mcp --doctor-smoke --provider codex
```

## Configuration

Preferred local config lives at `~/.agent-bridge-mcp/config.toml`:

```toml
workspaces = ["/path/to/workspaces"]
state_dir = "~/.agent-bridge-mcp/state"
claude_host_socket = "~/.agent-bridge-mcp/run/claude-host.sock"
max_active_tasks = 16
```

Config precedence is file < legacy env < CLI flags. `AGENT_BRIDGE_WORKSPACES`
remains supported as a platform path-list of allowed workspace roots, but it is
deprecated in favor of the config file or `--workspaces`. Task `cwd` values must
resolve inside one configured root.

`AGENT_BRIDGE_STATE_DIR` is optional. When omitted, Agent Bridge stores state in
`~/.agent-bridge-mcp/state`.

Workspace roots are cached at startup. After editing the config file, reload
without restarting the process:

```bash
agent-bridge-mcp reload
```

`reload` reads `state_dir/server.pid` and sends SIGHUP. A successful reload
updates workspace roots for new tasks; malformed config preserves the incumbent
roots and emits a JSON error log to stderr. All Agent Bridge logs are
newline-delimited JSON on stderr so stdout remains reserved for JSON-RPC traffic
and explicit CLI JSON output.

## MCP Client Configuration

For hosts that require MCP, use the `mcp-adapter` subcommand:

```json
{
  "mcpServers": {
    "agent-bridge": {
      "command": "agent-bridge-mcp",
      "args": ["mcp-adapter"],
      "env": {
        "AGENT_BRIDGE_WORKSPACES": "/path/to/workspaces",
        "AGENT_BRIDGE_STATE_DIR": "~/.agent-bridge-mcp/state"
      }
    }
  }
}
```

### Cursor

Cursor supports project-specific MCP servers in `.cursor/mcp.json` and global
MCP servers in `~/.cursor/mcp.json`. This repository checks in a project-level
Cursor config at `.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "agent-bridge": {
      "command": "agent-bridge-mcp",
      "args": ["mcp-adapter"],
      "env": {
        "AGENT_BRIDGE_WORKSPACES": "${workspaceFolder}",
        "AGENT_BRIDGE_STATE_DIR": "${env:HOME}/.agent-bridge-mcp/state"
      }
    }
  }
}
```

Use this flow for Cursor:

1. Build and install `agent-bridge-mcp` on `PATH`.
2. Open this repository in Cursor so `${workspaceFolder}` resolves to the repo
   root.
3. Confirm Cursor loads the `agent-bridge` server from `.cursor/mcp.json`.
4. Call `agent_delegate` with `prompt`, `cwd`, `mode`, and optional `policy`.
5. Read bounded evidence with `agent_evidence` when needed.

Use `~/.cursor/mcp.json` instead only when you want Agent Bridge available in
every Cursor project. Keep secrets out of checked-in MCP configs; use Cursor
environment interpolation such as `${env:NAME}` for machine-local values.

## Claude Host Runner

The Claude host-runner subcommand remains available for legacy diagnostics, but
provider task launches use ACP. New Claude provider setup should configure
`CLAUDE_ACP_BIN` and optional `CLAUDE_ACP_ARGS`, then run `agent-bridge-mcp
--doctor-smoke --provider claude` to validate launch readiness.

## Recommended Workflow

1. Run `--doctor-smoke` when setup or provider readiness is uncertain
   (`--provider <name>` to narrow the check).
2. Send an ACP `session/prompt` or call `agent_delegate` to run one routed
   provider turn.
3. Inspect the `routerResult` and run local verification yourself before
   trusting delegated output.
4. Use `agent_evidence` with the `evidenceRef` from `agent_delegate` to fetch
   bounded transcript, stdout, stderr, diff, summary, or changed-files sections.

Provider output is evidence, not proof. The caller remains responsible for tests,
lint, build, review, and cleanup.

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
- Managed worktrees are preserved for inspection until explicitly cleaned up.

## Release Artifacts

`.github/workflows/release-rust.yml` builds release artifacts for:

- Linux x64
- macOS x64
- macOS arm64

## License

MIT. See [LICENSE](LICENSE).
