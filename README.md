# Agent Bridge MCP

Rust stdio MCP server for spawning task-native coding agents from Codex.

This is a breaking redesign. The old `ask_*` and `dispatch_*` tools were removed.
The public surface is now a provider-neutral task lifecycle API modeled after
Claude/Codex-style delegated tasks.

This repo lives at:

```text
/Users/pedro/Development/agent-bridge-mcp
```

## Tools

- `providers_list`: list first-class providers and their capabilities.
- `providers_check`: check availability of each provider with `--version`, and optionally run startup smoke probes.
- `task_preview`: preview the command, args, and environment that would be used for a task without spawning it.
- `task_spawn`: start a background task and return a `taskId`.
- `task_list`: list tracked tasks.
- `task_status`: inspect one task lifecycle state.
- `task_wait`: wait for a task to reach a final state or return after a timeout.
- `task_logs`: read capped stdout/stderr slices; supports line cursors for incremental reads.
- `task_result`: read final result metadata, logs, git status, diff, changed files, and exit data.
- `task_stop`: terminate a running task.
- `task_remove`: remove a completed/stopped task; managed worktree cleanup is mandatory.

`task_spawn` returns immediately. Callers can poll `task_status`, `task_logs`, or
`task_result` with the returned `taskId`, or use `task_wait` to block until the
task completes or a timeout is reached. `task_preview` lets you inspect the
exact command, arguments, and environment keys before spawning.

Recommended caller workflow:

1. Call `providers_check` to catch missing or misconfigured CLIs before delegation. Use `smoke: true` when debugging provider startup, not just binary presence.
2. Call `task_preview` when debugging provider flags or cwd/env behavior.
3. Call `task_spawn` for the real task.
4. Call `task_wait` with a bounded `timeoutMs`; if it times out, use `task_logs`
   with line cursors to inspect progress without rereading the whole log.
5. Once the task is final, call `task_result` once for logs, git status, diff,
   changed files, exit metadata, and structured `errorType`.
6. Call `task_remove` intentionally after any managed worktree has been inspected.

## Providers

First-class providers:

- `claude`: local Claude Code through `claude-p` by default; set `CLAUDE_BIN` to use native `claude -p` instead when `CLAUDE_P_BIN` is not set.
- `cursor`: local Cursor Agent through `cursor-agent -p`.
- `kimi`: local Pi/Kimi through `pi -p`.
- `codex`: local Codex through `codex exec`.

Provider-specific capabilities, command construction, smoke probes, and
environment allowlists are implemented in the Rust provider module.

Supported modes:

- `research`: read/analyze only.
- `review`: read-only review.
- `implement`: write-capable implementation.
- `command`: bounded command-oriented work.

Provider/mode combinations are validated. For example, Cursor does not support
`command` mode in v1.

## Requirements

- Rust-built `agent-bridge-mcp` binary for the MCP runtime.
- `git` on `PATH`.
- `claude-p` on `PATH`, or set `CLAUDE_P_BIN` to an explicit wrapper path.
- Optional: set `CLAUDE_BIN` to use native `claude -p` instead of `claude-p`. `CLAUDE_P_BIN` takes precedence when both are set.
- `cursor-agent` on `PATH`, or set `CURSOR_AGENT_BIN`.
- `pi` on `PATH`, or set `PI_BIN`.
- `codex` on `PATH`, or set `CODEX_BIN`.

Supported first-release binary targets:

```text
macOS arm64
macOS x64
Linux x64
```

Provider CLIs may have narrower platform support than the bridge binary. Windows
is not a first-release target for the Rust migration.

## Install

Build and install the Rust binary from this repo:

```bash
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo build --release --bin agent-bridge-mcp
install -m 0755 target/release/agent-bridge-mcp ~/.local/bin/agent-bridge-mcp
```

Run the Rust-only stdio and lifecycle tests with:

```bash
cargo test
```

The temporary side-by-side Rust command is also buildable as:

```bash
cargo build --release --bin agent-bridge-mcp-rs
```

The MCP runtime command is the built Rust binary:

```bash
agent-bridge-mcp
```

Release artifacts are produced by `.github/workflows/release-rust.yml` for the
supported targets.

## Safety And State

- Public tool arguments are whitelisted; unknown fields are rejected.
- `cwd` is validated with `fs.realpath` to block symlink escapes.
- Set `AGENT_BRIDGE_ALLOWED_ROOT` to confine task cwd values to one workspace root.
- Prompts are capped at 100 KiB UTF-8.
- Task stdout/stderr, git status, and git diff are capped at 1 MiB each.
- Provider processes use ignored stdin unless a provider requires stdin prompt transport. Most providers receive a restricted environment allowlist.
- Claude provider runs through `/bin/zsh -lc` and sources `~/.zshenv`, `~/.zprofile`, and `~/.zshrc` before executing `claude-p` or native `claude`, so MCP behavior matches the terminal path by default. The shell script is constant; provider paths and cwd values are passed through `exec "$@"`, and prompt text is written to child stdin.
- Claude provider receives a focused CLI environment allowlist so Claude Code and `claude-p` can find auth/config without inheriting unrelated host secrets. The bridge strips injected `ANTHROPIC_BASE_URL` values that can point Claude at Codex-local proxy endpoints. `claude-p` is the default; set `CLAUDE_BIN` to opt into native `claude -p` when `CLAUDE_P_BIN` is unset.
- Codex provider passes `--config shell_environment_policy.inherit="all"` to `codex exec` so delegated Codex shell commands see the same tool `PATH` as the provider process.
- Active task state is persisted under `AGENT_BRIDGE_STATE_DIR`, defaulting to:

```text
~/.agent-bridge-mcp/state
```

State is written atomically. On MCP server restart, any previously running task is
marked `failed_stale` with `errorType: "stale"`; v1 does not reconnect to or
resume provider sessions. Treat stale tasks as needing manual inspection and a
fresh spawn.

Task states:

```text
queued
running
succeeded
failed
stopped
failed_stale
removed
```

Final task payloads include `isFinal`, `phase`, and `durationMs` where timing
data is available. Failure payloads keep the human-readable `error` string and
also include `errorType`, such as `timeout`, `provider_exit_error`,
`provider_start_error`, `provider_output_error`, `stopped`, or `stale`.

If a provider appears stalled, call `task_wait` with a short timeout and then
`task_logs` with the latest line cursors. If there is still no useful output,
call `task_stop`; the stopped task remains inspectable through `task_result`.

## Claude Troubleshooting

`providers_check` without `smoke` proves only that the selected Claude binary answers `--version`; it reports `startupVerified: false`. Use `providers_check` with `smoke: true` when Claude hangs, exits without a result, emits terminal noise, or appears healthy but cannot complete tasks.

Claude smoke checks and failed Claude task results include additive `diagnostic` fields with a stable `failureCategory`, selected `commandKind`, selected `commandPath`, timeout, exit metadata, and capped stdout/stderr excerpts. Excerpts are capped and redact prompt content and known sensitive prompt tokens.

Selection rules:

- Set `CLAUDE_P_BIN` to force a specific `claude-p` wrapper.
- Set `CLAUDE_BIN` to use native `claude -p` when `CLAUDE_P_BIN` is not set.
- If a `claude-p` smoke probe fails and `CLAUDE_BIN` is configured, diagnostics recommend trying native `claude -p`; the bridge does not silently switch commands.

`claude-p` is an external compatibility wrapper around interactive Claude Code. Its README describes PTY startup handling, Stop hook result capture, `--input-file`, stdin prompt input, and caveats that Claude Code terminal or hook behavior changes can break the wrapper: <https://github.com/smithersai/claude-p#readme>. Native Claude Code CLI reference for `claude -p`, `--output-format`, and stdin input formats is available at <https://code.claude.com/docs/en/cli-reference>.

## Isolation

`task_spawn` supports:

- `isolation: "none"`: run in the validated `cwd`.
- `isolation: "worktree"`: create a unique git worktree under the state directory.

Managed worktrees are preserved after task completion for inspection. `task_remove`
must successfully run `git worktree remove -f <worktree>` before removing the task
record. If cleanup fails, the task remains tracked.

## Codex MCP Config

Use the installed Rust binary:

```json
{
  "mcpServers": {
    "agent-bridge": {
      "command": "agent-bridge-mcp",
      "args": [],
      "env": {
        "AGENT_BRIDGE_ALLOWED_ROOT": "/Users/pedro/Development/agent-bridge-mcp"
      }
    }
  }
}
```

Or register it with Codex:

```bash
codex mcp add \
  --env AGENT_BRIDGE_ALLOWED_ROOT=/Users/pedro/Development/agent-bridge-mcp \
  agent-bridge \
  -- agent-bridge-mcp
```

## Examples

List providers:

```json
{
  "name": "providers_list",
  "arguments": {}
}
```

Check which providers are available:

```json
{
  "name": "providers_check",
  "arguments": {}
}
```

Run provider startup smoke probes:

```json
{
  "name": "providers_check",
  "arguments": {
    "smoke": true,
    "timeoutMs": 10000
  }
}
```

Without `smoke`, `providers_check` reports `probe: "version"` and `startupVerified: false`.
With `smoke: true`, it reports `probe: "version+smoke"` and only sets
`startupVerified: true` after a short noninteractive provider task exits
successfully.

Preview a task before spawning:

```json
{
  "name": "task_preview",
  "arguments": {
    "provider": "codex",
    "mode": "review",
    "prompt": "Review the parser for edge cases.",
    "cwd": "/Users/pedro/Development/agent-bridge-mcp"
  }
}
```

Spawn a Claude implementation task:

```json
{
  "name": "task_spawn",
  "arguments": {
    "provider": "claude",
    "mode": "implement",
    "title": "Fix parser bug",
    "prompt": "Reproduce and fix the parser bug described in the failing tests. Keep the change minimal and report verification evidence.",
    "cwd": "/Users/pedro/Development/agent-bridge-mcp",
    "timeoutSeconds": 600,
    "isolation": "worktree"
  }
}
```

Poll task status:

```json
{
  "name": "task_status",
  "arguments": {
    "taskId": "task_..."
  }
}
```

Read logs incrementally:

```json
{
  "name": "task_logs",
  "arguments": {
    "taskId": "task_...",
    "stdoutLine": 10,
    "stderrLine": 2
  }
}
```

Wait for a task to complete (up to 60s):

```json
{
  "name": "task_wait",
  "arguments": {
    "taskId": "task_...",
    "timeoutMs": 60000
  }
}
```

Read final result:

```json
{
  "name": "task_result",
  "arguments": {
    "taskId": "task_..."
  }
}
```

Remove a finished task and clean its managed worktree:

```json
{
  "name": "task_remove",
  "arguments": {
    "taskId": "task_..."
  }
}
```

Run tests:

```bash
cargo test
```
