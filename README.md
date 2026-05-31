# Agent Bridge MCP

Dependency-free stdio MCP server for spawning task-native coding agents from Codex.

This is a breaking redesign. The old `ask_*` and `dispatch_*` tools were removed.
The public surface is now a provider-neutral task lifecycle API modeled after
Claude/Codex-style delegated tasks.

This repo lives at:

```text
/Users/pedro/Development/agent-bridge-mcp
```

## Tools

- `providers_list`: list first-class providers and their capabilities.
- `task_spawn`: start a background task and return a `taskId`.
- `task_list`: list tracked tasks.
- `task_status`: inspect one task lifecycle state.
- `task_logs`: read capped stdout/stderr slices.
- `task_result`: read final result metadata, logs, git status, diff, changed files, and exit data.
- `task_stop`: terminate a running task.
- `task_remove`: remove a completed/stopped task; managed worktree cleanup is mandatory.

`task_spawn` returns immediately. Callers must poll `task_status`, `task_logs`, or
`task_result` with the returned `taskId`.

## Providers

First-class providers:

- `claude`: local Claude Code through `claude-p` by default; set `CLAUDE_BIN` to use native `claude -p` instead.
- `cursor`: local Cursor Agent through `cursor-agent -p`.
- `kimi`: local Pi/Kimi through `pi -p`.
- `codex`: local Codex through `codex exec`.

Supported modes:

- `research`: read/analyze only.
- `review`: read-only review.
- `implement`: write-capable implementation.
- `command`: bounded command-oriented work.

Provider/mode combinations are validated. For example, Cursor does not support
`command` mode in v1.

## Requirements

- Node.js 24 or newer.
- `claude-p` on `PATH`, or set `CLAUDE_P_BIN`.
- Optional: set `CLAUDE_BIN` to use native `claude -p` instead of `claude-p`.
- `cursor-agent` on `PATH`, or set `CURSOR_AGENT_BIN`.
- `pi` on `PATH`, or set `PI_BIN`.
- `codex` on `PATH`, or set `CODEX_BIN`.

## Install

From this repo:

```bash
npm test
npm run pack:local
```

The local package artifact is written to:

```text
outputs/agent-bridge-mcp-0.1.0.tgz
```

Install from the tarball elsewhere with:

```bash
npm install -g /Users/pedro/Development/agent-bridge-mcp/outputs/agent-bridge-mcp-0.1.0.tgz
```

Then the executable is:

```bash
agent-bridge-mcp
```

## Safety And State

- Public tool arguments are whitelisted; unknown fields are rejected.
- `cwd` is validated with `fs.realpath` to block symlink escapes.
- Set `AGENT_BRIDGE_ALLOWED_ROOT` to confine task cwd values to one workspace root.
- Prompts are capped at 100 KiB UTF-8.
- Task stdout/stderr, git status, and git diff are capped at 1 MiB each.
- Provider processes use ignored stdin and timeouts. Most providers receive a restricted environment allowlist.
- Claude provider receives the local CLI environment so Claude Code and `claude-p` can find the same auth/config as your terminal, but the bridge strips injected `ANTHROPIC_BASE_URL` values that can point Claude at Codex-local proxy endpoints. `claude-p` is the default; set `CLAUDE_BIN` to opt into native `claude -p`.
- Active task state is persisted under `AGENT_BRIDGE_STATE_DIR`, defaulting to:

```text
~/.agent-bridge-mcp/state
```

State is written atomically. On MCP server restart, any previously running task is
marked `failed_stale`; v1 does not reconnect to or resume provider sessions.

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

## Isolation

`task_spawn` supports:

- `isolation: "none"`: run in the validated `cwd`.
- `isolation: "worktree"`: create a unique git worktree under the state directory.

Managed worktrees are preserved after task completion for inspection. `task_remove`
must successfully run `git worktree remove -f <worktree>` before removing the task
record. If cleanup fails, the task remains tracked.

## Codex MCP Config

Use an absolute path to `src/server.mjs`:

```json
{
  "mcpServers": {
    "agent-bridge": {
      "command": "node",
      "args": [
        "/Users/pedro/Development/agent-bridge-mcp/src/server.mjs"
      ],
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
  -- node /Users/pedro/Development/agent-bridge-mcp/src/server.mjs
```

## Examples

List providers:

```json
{
  "name": "providers_list",
  "arguments": {}
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
npm test
```
