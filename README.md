# Agent Bridge MCP

Agent Bridge MCP is a Rust stdio MCP server for delegating bounded work from one
agent client to local provider agents such as Claude Code, Codex, Cursor Agent,
Kimi/Pi, and Google Antigravity CLI.

It exposes a provider-neutral lifecycle so a caller can preview, start, observe,
inspect, stop, and remove delegated tasks while keeping the main agent
responsible for verification.

## What It Provides

A consolidated, lean eight-tool surface:

- `providers_list` — first-class providers and their agent capabilities.
- `doctor` — setup, workspace, state, client config, binary freshness, and
  provider/host-runner readiness. `focus: "providers"` runs a readiness-only
  check; `smoke: true` verifies launchability.
- `agent_spawn` — start a provider agent. `dryRun: true` previews the launch
  (command, cwd, environment, profile, isolation) without spawning.
- `agent_observe` — primary progress path. `until: "final"` blocks to finality,
  `limit: 0` returns lifecycle state only, and the `events` stream is the agent
  transcript.
- `agent_result` — final evidence. Returns the review packet and changed files
  by default; request `sections: ["stdout","stderr","diff","transcript"]` to
  fetch raw evidence on demand (paged with `maxBytes`/`stdoutLine`/`stderrLine`/
  transcript `cursor`/`limit`).
- `agent_list` — bounded active/recent agent summaries.
- `agent_stop` — terminate a running agent that is no longer useful.
- `agent_remove` — remove a finished/stopped agent after result inspection.

Responses are lean by default (each field appears once, no GUI presentation
chrome); pass `verbosity: "detailed"` on `agent_observe`/`agent_result` for debug
metadata. Tools carry MCP `annotations` (`readOnlyHint`/`destructiveHint`) so
Tool-Search-capable clients can tier and defer them.

It also provides MCP self-description through prompts and guidance resources
(including `agent-bridge://guidance/code-execution`), and deterministic
fake-provider tests that do not require paid model access, network access,
provider credentials, or host keychain permissions.

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

To dogfood provider/profile behavior with real local providers, see
[`docs/WORKFLOWS/dogfood-provider-profiles.md`](docs/WORKFLOWS/dogfood-provider-profiles.md).

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

Run `doctor` with `focus: "providers"` and `smoke: true` when validating a
Claude setup.

## Recommended Workflow

1. Call `doctor` when setup or provider readiness is uncertain
   (`focus: "providers"` for a readiness-only check; `smoke: true` when launch
   readiness matters).
2. Call `agent_spawn` to start a bounded provider task (`dryRun: true` to preview
   the launch without spawning).
3. Use `agent_observe` to monitor progress and follow the `next` action list
   (`until: "final"` to block to finality, `limit: 0` for a quick state check).
4. Use `agent_result` after finalization for the review packet and changed
   files; request `sections` for raw logs, diff, or transcript evidence.
5. Run local project verification yourself before trusting delegated output.
6. Call `agent_remove` intentionally after inspecting any managed worktree.

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

The surface was consolidated from fourteen tools to eight; six tools were folded
into the retained ones via parameters. Migrate as follows:

| Removed tool | Replacement |
| --- | --- |
| `agent_preview` | `agent_spawn` with `dryRun: true` |
| `agent_status` | `agent_observe` with `limit: 0` |
| `agent_wait` | `agent_observe` with `until: "final"`, `timeoutMs` |
| `agent_transcript` | `agent_observe` `events` (with `cursor`/`limit`) |
| `agent_logs` | `agent_result` with `sections: ["stdout","stderr"]` and line pagination |
| `providers_check` | `doctor` with `focus: "providers"` (plus `smoke`/`providers`/`timeoutMs` as before) |

Responses are also leaner: `agent_observe`/`agent_result` return a single `next`
action list (the previous duplicated `nextActions`/`presentation`/`progress`
copies and the GUI `presentation` object are gone). Pass `verbosity: "detailed"`
to re-add debug metadata, and request `agent_result` `sections` for raw evidence.

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
