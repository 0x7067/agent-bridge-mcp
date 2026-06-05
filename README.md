# Agent Bridge MCP

Rust stdio MCP server for spawning provider agents from Codex.

This is a breaking redesign. The old `ask_*` and `dispatch_*` tools were removed.
The public surface is now a provider-neutral agent launch API backed by a task
lifecycle modeled after Claude/Codex-style delegated tasks.

This repo lives at:

```text
/Users/pedro/Development/agent-bridge-mcp
```

## Tools

- `providers_list`: list first-class providers and their capabilities.
- `providers_check`: check availability of each provider with `--version`, and optionally run startup smoke probes.
- `doctor`: return a structured setup report for server, workspace, state, providers, Claude host-runner, and recommendations.
- `agent_preview`: preview the command, args, and environment that would be used for a task without spawning it.
- `agent_spawn`: start a provider agent and return the `taskId` used by lifecycle tools.
- `agent_list`: list active/recent provider agents with bounded native-client presentation summaries.
- `agent_status`: inspect one task lifecycle state.
- `agent_wait`: wait for a task to reach a final state or return after a timeout.
- `agent_logs`: read capped stdout/stderr slices; supports line cursors for incremental reads.
- `agent_transcript`: read bounded normalized run transcript events with cursor/limit controls.
- `agent_observe`: wait for new transcript/lifecycle events or finalization with bounded long polling.
- `agent_result`: read final result metadata, logs, git status, diff, changed files, and exit data.
- `agent_stop`: terminate a running task.
- `agent_remove`: remove a completed/stopped task; managed worktree cleanup is mandatory.

`agent_spawn` returns immediately. Callers can poll `agent_status`, `agent_logs`, or
`agent_result` with the returned `taskId`, or use `agent_wait` to block until the
task completes or a timeout is reached. `agent_preview` lets you inspect the
exact command, arguments, selected launch profile, profile diagnostics, and
environment keys before spawning.

Recommended caller workflow:

1. Call `doctor` first when setup, workspace, state, provider, or Claude host-runner readiness is uncertain.
2. Call `providers_check` to catch missing or misconfigured CLIs before delegation. Use `smoke: true` when debugging provider startup, not just binary presence.
3. Call `agent_preview` when debugging provider flags or cwd/env behavior.
4. Call `agent_spawn` for the real provider agent.
5. Call `agent_list` or `agent_status` to read each task's `presentation` metadata
   for native-client display title, status tone, result availability, structured
   lifecycle actions, ranked `nextActions`, and unavailable reply/resume controls.
6. Call `agent_observe` with a bounded `timeoutMs` and cursor to wait for new
   transcript/lifecycle events. If it times out, use `agent_logs` with line
   cursors and `agent_transcript` with cursors to inspect progress without
   rereading the whole run.
7. Once the task is final, call `agent_result` once for logs, git status, diff,
   changed files, exit metadata, structured `errorType`, and the derived
   `reviewPacket` inspection summary.
8. Call `agent_remove` intentionally after any managed worktree has been inspected.

For setup troubleshooting, `doctor` is the broad first check:

```json
{
  "name": "doctor",
  "arguments": {
    "cwd": "/path/to/workspace",
    "providers": ["claude", "codex"]
  }
}
```

Use `summary.status` to triage: `error` means fix workspace, state, or host-runner blockers first; `warning` usually means a provider or optional readiness concern needs follow-up; `ok` means the bridge setup checks did not find a setup problem. `doctor` does not verify delegated task output or project tests.
Use `launchReadiness` separately: version-only provider checks can leave providers
available but not startup-verified or launchable. When startup readiness matters,
follow the structured recommendation to run `providers_check` with `smoke: true`
for the selected providers.
Use `clients` separately for static user-level MCP client configuration
diagnostics. It inspects only `~/.codex/config.toml`, `~/.claude.json`, and
`~/.cursor/mcp.json`; it does not search project-level overrides, edit config
files, run client CLIs, spawn providers, or prove MCP startup. Registered Codex
and Claude entries include shell follow-up guidance such as `codex mcp list` or
`claude mcp list` so the caller can verify the client separately. Client config
issues are reported under `clients` and low-severity recommendations; they do not
change `summary.status` because the bridge cannot reliably know which client is
invoking it over stdio.
Use `binary` separately for running/installed/release binary freshness
diagnostics. It compares file metadata and a bounded diagnostic fingerprint for
the running executable, the installed binary path, and the release candidate
path. It does not build, copy, install, or delete binaries. Set
`AGENT_BRIDGE_INSTALLED_BIN` or `AGENT_BRIDGE_RELEASE_BIN` only when the default
paths do not match your setup; otherwise the installed path defaults to
`~/.local/bin/agent-bridge-mcp` and the release candidate defaults to
`<doctor cwd>/target/release/agent-bridge-mcp`.

Real-world delegation workflow:

- Treat provider output as evidence for the main Codex thread, not as final verification. Inspect the final report, logs, `gitStatus`, `diff`, `changedFiles`, and exit metadata before using the result.
- Use the `presentation` object on `agent_list`, `agent_status`, and `agent_result` for native-feeling UI summaries. `presentation.nextActions`, top-level `nextActions`, and `reviewPacket.nextActions` provide ranked follow-up calls with arguments and safety classifications. `verificationStatus: "not_verified"` means provider completion is not project verification.
- `agent_list` is the bounded active/recent presentation list for harnesses. It does not expose raw full-history task registry inspection as a separate public MCP workflow.
- Render `reply` and `resume` presentation actions as unavailable in v1. Provider tasks are batch lifecycle tasks, not interactive resumable conversations.
- Provider capability `presentationActions` keys are camelCase capability names; per-task `presentation.actions[].id` values are snake_case lifecycle action ids. Treat them as related but separate surfaces.
- Treat `providers_list.readiness` as non-blocking discovery. It starts as `state: "stale"` and `launchable: false`; run `providers_check` with `smoke: true` to mark a provider `ready` and launchable or `failed` with diagnostics.
- Removed tasks are excluded from native presentation lists before lifecycle actions are rendered. Present lifecycle controls only for inspectable task records.
- Use `agent_transcript` when analyzing provider behavior, comparing providers, or checking whether a final or partial provider result was detected.
- Use profile `bridge` for normal Agent Bridge task guidance. Use profile `bare` for paired experiments with compact bridge-owned prompts and provider-specific reduced configuration; inspect `profileDiagnostics` because reductions vary by provider.
- Keep the main thread responsible for project gates. Run the relevant tests, lint, typecheck, build, or OpenSpec validation before claiming the requested work is complete.
- Use `research` and `review` modes for read-only analysis, second opinions, and plan critique.
- Use `command` mode only for bounded command-oriented work where the prompt clearly names the command goal and expected evidence.
- Use `implement` mode with `isolation: "worktree"` by default so provider edits land in a managed git worktree that can be inspected before integration.
- After inspecting a final managed-worktree task, call `agent_remove` intentionally. Cleanup is explicit so callers can review generated files and diffs before the worktree is removed.

If a provider appears stalled:

1. Call `agent_observe` with a bounded timeout and cursor to wait for new transcript/lifecycle events.
2. Call `agent_logs` with `stdoutLine` and `stderrLine` cursors to inspect new output without rereading the whole log.
3. If the task is still not useful, call `agent_stop`.
4. Call `agent_result` on the stopped task to inspect logs, exit metadata, diagnostics, and any partial git state.
5. Decide in the main thread whether to discard, re-run with a narrower prompt, or manually continue from the inspected state.

## MCP Self-Description

In addition to tools, the server exposes MCP prompts and resources so clients can
discover how to use Agent Bridge safely:

- `prompts/list` exposes workflow templates for delegated review, isolated
  implementation, result inspection, and stalled task recovery.
- `prompts/get` returns user-message guidance for the selected workflow.
- `resources/list` exposes static `agent-bridge://guidance/...` resources for
  caller workflow, safety guidance, provider capabilities, Claude host-runner
  lifecycle, and dogfood workflows.
- `resources/read` returns those resources as `text/markdown` from a hardcoded
  allowlist. It does not map resource URIs to local files.
- `initialize` returns concise Agent Bridge workflow instructions. JSON-returning
  tools include `structuredContent` and stable top-level `outputSchema` metadata
  for core lifecycle outputs.

Client behavior is host-dependent. Tool schemas are the most likely surface to
be visible to the model automatically. Prompts are normally user-selected
workflow templates. Resources may be shown in a picker, searched, or included
automatically only if the host implements those heuristics. Clients that ignore
`initialize.instructions`, `structuredContent`, output schemas, or `nextActions`
can still follow the manual lifecycle through `doctor`, `providers_check`,
`agent_spawn`, `agent_list`, `agent_wait`, `agent_logs`, `agent_transcript`, `agent_result`, and
`agent_remove`.

Protocol-level MCP Tasks are separate from Agent Bridge lifecycle tools. The
stable Agent Bridge workflow uses `agent_spawn`, `agent_list`, and `agent_*` lifecycle tools today. MCP task primitives are
experimental/extension-gated and should be used only after negotiated host and
client capability support is explicitly implemented and advertised. `doctor`
includes `taskExtensionReadiness` as passive diagnostic evidence about task-like
client metadata observed during `initialize` or request `_meta`; it always reports
`serverAdvertisesTasks: false` in this release. Even when a client is classified
as `extension_capable`, protocol-level `tasks/*`, `CreateTaskResult`, protocol
task listing, cancellation, and notifications remain unavailable. The existing
compatibility memo at
`openspec/changes/explore-mcp-task-support/compatibility-memo.md` remains the
design reference for future protocol task support.

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

Launch profiles are explicit task inputs. Omitted `profile` defaults to
`bridge`, which uses the normal Agent Bridge prompt and provider adapter
behavior. `bare` uses compact bridge-owned instructions plus provider-specific
reduced configuration where available; inspect `profileDiagnostics` and
`providers_list.reducedConfiguration` for the actual applied, unsupported, and
best-effort reductions.

## Requirements

- Rust-built `agent-bridge-mcp` binary for the MCP runtime.
- `git` on `PATH`.
- `claude-p` on `PATH`, or set `CLAUDE_P_BIN` to an explicit wrapper path.
- Optional: set `AGENT_BRIDGE_CLAUDE_HOST_SOCKET` to route Claude provider calls through the bridge's Claude host runner. This is required when Claude Code auth is stored in macOS Keychain and the MCP server runs inside a sandbox that cannot access Keychain.
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
- Set `AGENT_BRIDGE_WORKSPACES` to confine task cwd values to one or more workspace roots. It uses the platform path-list separator, such as `:` on macOS/Linux.
- Prompts are capped at 100 KiB UTF-8.
- Task stdout/stderr, git status, and git diff are capped at 1 MiB each.
- Provider processes use ignored stdin unless a provider requires stdin prompt transport. Most providers receive a restricted environment allowlist.
- Claude provider runs through `/bin/zsh -flc` and manually sources `~/.zshenv`, `~/.zprofile`, and `~/.zshrc` with stdin redirected from `/dev/null` before executing `claude-p` or native `claude`, so MCP behavior matches the terminal path without letting startup files consume provider prompts. The shell script is constant; provider paths and cwd values are passed through `exec "$@"`, and prompt text is written to child stdin.
- Claude provider receives a focused CLI environment allowlist so Claude Code and `claude-p` can find auth/config without inheriting unrelated host secrets. The bridge strips injected `ANTHROPIC_BASE_URL` values that can point Claude at Codex-local proxy endpoints. `claude-p` is the default.
- When `AGENT_BRIDGE_CLAUDE_HOST_SOCKET` is configured, Claude provider smoke checks and tasks use a local Unix-socket host runner. The runner reconstructs the `claude-p` command from structured request fields and executes it outside Codex's sandbox so macOS Keychain-backed Claude Code auth remains available.
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

`agent_result` also includes `reviewPacket`, an additive summary derived from the
existing result fields. It reports status, finality, provider/mode, cwd,
changed files, whether git state changed, exit/error metadata, truncation flags,
diagnostics when present, and recommended next actions. Treat it as an
inspection aid, not verification; the main caller still runs the relevant tests,
lint, typecheck, build, or OpenSpec validation before claiming completion.

If a provider appears stalled, call `agent_wait` with a short timeout and then
`agent_logs` with the latest line cursors. If there is still no useful output,
call `agent_stop`; the stopped task remains inspectable through `agent_result`.

## Live Smoke Checks

Default automated verification uses deterministic fake providers. It does not
require live Claude, Cursor, Kimi, Codex, network access, paid model usage, or
host keychain permissions.

Run live provider smoke checks only when you intentionally want to exercise the
installed local CLIs. For a focused check, filter to one provider:

```json
{
  "name": "providers_check",
  "arguments": {
    "smoke": true,
    "providers": ["cursor"]
  }
}
```

All-provider smoke checks use bounded concurrent probes with provider-specific
default budgets. The current defaults are 20s for Codex, 30s for Claude, 45s for
Kimi, and 60s for Cursor, under a 110s aggregate call budget. The earlier live
investigation saw successful task-path probes at roughly 11.5s for Codex, 20.5s
for Claude, 33.2s for Kimi, and 52.1s for Cursor.

Use explicit budgets when investigating a slow first run:

```json
{
  "name": "providers_check",
  "arguments": {
    "smoke": true,
    "providers": ["claude", "codex"],
    "aggregateTimeoutMs": 110000,
    "providerTimeoutMs": {
      "claude": 30000,
      "codex": 20000
    }
  }
}
```

`timeoutMs` remains a per-provider fallback for existing callers. Set
`AGENT_BRIDGE_SMOKE_CONCURRENCY=1` to run smoke probes sequentially while
debugging local resource contention.

For a minimal read-only task smoke, use `research` or `review`, a short timeout,
and `isolation: "none"` unless you specifically want a managed worktree:

```json
{
  "name": "agent_spawn",
  "arguments": {
    "provider": "codex",
    "mode": "review",
    "prompt": "Inspect the repository at a high level and return one sentence: AGENT_BRIDGE_LIVE_TASK_OK. Do not edit files.",
    "cwd": "/Users/pedro/Development/agent-bridge-mcp",
    "timeoutSeconds": 120,
    "isolation": "none"
  }
}
```

Then call `agent_wait` with a bounded timeout and inspect `agent_result`. Live
smoke prompts should be small, read-only, and explicit about not editing files.

## Claude Troubleshooting

`providers_check` without `smoke` proves only that the selected Claude binary answers `--version`; it reports `startupVerified: false`. Use `providers_check` with `smoke: true` when Claude hangs, exits without a result, emits terminal noise, or appears healthy but cannot complete tasks.

Claude smoke checks and failed Claude task results include additive `diagnostic` fields with a stable `failureCategory`, selected `commandKind`, selected `commandPath`, timeout, exit metadata, and capped stdout/stderr excerpts. Excerpts are capped and redact prompt content and known sensitive prompt tokens.

Selection rules:

- Set `CLAUDE_P_BIN` to force a specific `claude-p` wrapper.
- Set `AGENT_BRIDGE_CLAUDE_HOST_SOCKET` to force Claude smoke checks and tasks through the host runner.
- If Claude Code is logged in through macOS Keychain, use the host runner; sandboxed MCP child processes may not be able to read that login state.
- If the host runner returns `workspace_policy_mismatch`, restart the host runner after changing `AGENT_BRIDGE_WORKSPACES` or Codex workspace settings.

Start the host runner outside the Codex sandbox:

```bash
mkdir -p ~/.agent-bridge-mcp/run
AGENT_BRIDGE_WORKSPACES="/Users/pedro/Development:/Users/pedro/Documents" \
  agent-bridge-mcp claude-host-runner ~/.agent-bridge-mcp/run/claude-host.sock
```

For a detached local session, run it under `screen`:

```bash
screen -dmS agent-bridge-claude-host \
  env AGENT_BRIDGE_WORKSPACES="/Users/pedro/Development:/Users/pedro/Documents" \
  agent-bridge-mcp claude-host-runner ~/.agent-bridge-mcp/run/claude-host.sock
```

Then expose the same socket to the MCP server:

```toml
[mcp_servers.agent-bridge.env]
AGENT_BRIDGE_WORKSPACES = "/Users/pedro/Development:/Users/pedro/Documents"
AGENT_BRIDGE_STATE_DIR = "/Users/pedro/.agent-bridge-mcp/state"
AGENT_BRIDGE_CLAUDE_HOST_SOCKET = "/Users/pedro/.agent-bridge-mcp/run/claude-host.sock"
```

After reloading MCP configuration, `agent_preview` for Claude includes `launchStrategy: "host_runner"` and Claude smoke diagnostics include the same launch strategy.

Host-runner lifecycle checklist:

1. Start `agent-bridge-mcp claude-host-runner <socket>` outside the Codex sandbox with the same `AGENT_BRIDGE_WORKSPACES` value as the MCP server.
2. Confirm readiness with a Claude-only `providers_check` smoke or a direct host-runner protocol `ping` request when debugging the socket itself.
3. Restart the host runner after changing `AGENT_BRIDGE_WORKSPACES`; `workspace_policy_mismatch` means the runner and MCP server disagree about workspace policy.
4. Stop the runner with SIGTERM or SIGINT so it stops accepting new connections and terminates active Claude children.
5. If the runner reports `host_runner_unavailable`, inspect or restart the runner; the bridge intentionally does not silently fall back to sandboxed Claude execution.

`claude-p` is an external compatibility wrapper around interactive Claude Code. Its README describes PTY startup handling, Stop hook result capture, `--input-file`, stdin prompt input, and caveats that Claude Code terminal or hook behavior changes can break the wrapper: <https://github.com/smithersai/claude-p#readme>. Native Claude Code CLI reference for `claude -p`, `--output-format`, and stdin input formats is available at <https://code.claude.com/docs/en/cli-reference>.

## Isolation

`agent_spawn` and legacy `agent_spawn` support:

- `isolation: "none"`: run in the validated `cwd`.
- `isolation: "worktree"`: create a unique git worktree under the state directory.

Managed worktrees are preserved after task completion for inspection. `agent_remove`
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
        "AGENT_BRIDGE_WORKSPACES": "/Users/pedro/Development"
      }
    }
  }
}
```

Or register it with Codex:

```bash
codex mcp add \
  --env AGENT_BRIDGE_WORKSPACES=/Users/pedro/Development \
  agent-bridge \
  -- agent-bridge-mcp
```

## Dogfood Workflows

Use these local workflows intentionally; they are not part of the default test
suite because they may require installed provider CLIs, auth, network access, or
paid model usage.

- Read-only review: use `agent_spawn` with `review` or `research`, `isolation: "none"`, a
  small prompt, and bounded observation. Inspect `agent_result.reviewPacket`, logs,
  diagnostics, git status, diff, changed files, and exit metadata.
- Native agent presentation: call `agent_list` with default arguments to show
  active provider agents first and recent final agents second.
- Isolated implementation: use `agent_spawn` with `implement` and `isolation: "worktree"`.
  Inspect the managed worktree, `reviewPacket`, `gitStatus`, `gitDiff`, and
  `changedFiles`; run verification in the main caller; call `agent_remove` only
  after review.
- Stalled task recovery: use bounded `agent_observe` calls, incremental `agent_logs`
  cursors, `agent_status`, then `agent_stop` if the task is no longer useful.
  Inspect final `agent_result` before deciding to rerun or continue manually.
- Codex sandbox and approval denials: if logs mention `patch rejected`,
  sandbox denial, approval denial, outside of the project, or out-of-workspace
  writes, inspect `cwd`, workspace policy, prompt scope, isolation strategy,
  diagnostics, and final `agent_result` before retrying. Prefer narrowing the
  prompt or using managed worktree isolation over loosening sandbox permissions.
- Provider comparison: run equivalent read-only prompts against selected
  providers, optionally in paired `bridge` and `bare` profiles, and compare
  `reviewPacket`, transcript events, logs, diagnostics, exit metadata,
  profile diagnostics, and provider prose as evidence.

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
Provider discovery is intentionally non-blocking: `providers_list` reports static
capabilities and a stale/non-launchable readiness snapshot until `providers_check`
refreshes it. Version-only checks do not imply a provider can launch tasks.

Preview a task before spawning:

```json
{
  "name": "agent_preview",
  "arguments": {
    "provider": "codex",
    "mode": "review",
    "prompt": "Review the parser for edge cases.",
    "cwd": "/Users/pedro/Development/agent-bridge-mcp"
  }
}
```

Preview a reduced `bare` profile task:

```json
{
  "name": "agent_preview",
  "arguments": {
    "provider": "codex",
    "mode": "review",
    "profile": "bare",
    "prompt": "Review the parser for edge cases.",
    "cwd": "/Users/pedro/Development/agent-bridge-mcp"
  }
}
```

Spawn a Claude implementation agent:

```json
{
  "name": "agent_spawn",
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

List active/recent agents for native-client presentation:

```json
{
  "name": "agent_list",
  "arguments": {}
}
```

Filter presentation summaries:

```json
{
  "name": "agent_list",
  "arguments": {
    "provider": ["cursor"],
    "mode": ["review"],
    "cwd": "/Users/pedro/Development/agent-bridge-mcp",
    "titleContains": "parser",
    "limit": 10
  }
}
```

Poll task status:

```json
{
  "name": "agent_status",
  "arguments": {
    "taskId": "task_..."
  }
}
```

Read logs incrementally:

```json
{
  "name": "agent_logs",
  "arguments": {
    "taskId": "task_...",
    "stdoutLine": 10,
    "stderrLine": 2
  }
}
```

Read transcript events incrementally:

```json
{
  "name": "agent_transcript",
  "arguments": {
    "taskId": "task_...",
    "cursor": 0,
    "limit": 100
  }
}
```

Observe progress or finalization:

```json
{
  "name": "agent_observe",
  "arguments": {
    "taskId": "task_...",
    "cursor": 0,
    "limit": 100,
    "timeoutMs": 30000
  }
}
```

Wait for a task to complete (up to 60s):

```json
{
  "name": "agent_wait",
  "arguments": {
    "taskId": "task_...",
    "timeoutMs": 60000
  }
}
```

Read final result:

```json
{
  "name": "agent_result",
  "arguments": {
    "taskId": "task_..."
  }
}
```

Remove a finished task and clean its managed worktree:

```json
{
  "name": "agent_remove",
  "arguments": {
    "taskId": "task_..."
  }
}
```

Run tests:

```bash
cargo test
```
