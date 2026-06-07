# External Integrations

**Last verified:** 2026-06-07

## Integration Map

Agent Bridge MCP is a **desktop stdio server** with no traditional cloud service integrations. Its "external systems" are the provider CLI binaries installed on the host machine and the local operating-system facilities it uses.

| Service | Purpose | Protocol | Module Using It | Critical? |
|---------|---------|----------|-----------------|-----------|
| Operating System (POSIX) | Process management, PTYs, signals, filesystem | syscalls / libc | `task/supervision.rs`, `runtime.rs`, `claude_interactive/pty.rs` | Yes — cannot spawn or supervise without it |
| Git CLI | Repository introspection (diff, status, worktrees) | Shell invocation | `task/complete.rs` | Yes — worktree isolation and review packets depend on it |
| Claude Code CLI | Interactive agent execution via PTY | PTY + Unix socket (host runner) | `claude_host.rs`, `claude_interactive/` | No — only when Claude provider is chosen |
| Codex CLI | Noninteractive agent execution | Fork/exec + pipes | `provider.rs`, `task/spawn.rs` | No — only when Codex provider is chosen |
| Cursor Agent CLI | Noninteractive agent execution | Fork/exec + pipes | `provider.rs`, `task/spawn.rs` | No — only when Cursor provider is chosen |
| Kimi/Pi CLI | Noninteractive agent execution | Fork/exec + pipes | `provider.rs`, `task/spawn.rs` | No — only when Kimi provider is chosen |
| Antigravity CLI | Noninteractive agent execution | Fork/exec + pipes | `provider.rs`, `task/spawn.rs` | No — only when Antigravity provider is chosen |
| Filesystem | Persistence (registry, logs, transcripts, diffs) | POSIX fs ops | `task/registry.rs`, `task/complete.rs` | Yes — loss of state dir means lost task history |

## Integration Details

### Operating System (POSIX)

- **Purpose:** Process forking, signal delivery, pseudo-terminal allocation, process-group management.
- **Configuration:** No env vars; compiled against `libc` and `pty-process` crate.
- **Error handling:** Fatal on syscall failure; panic hook attempts SIGTERM cleanup of registered PIDs.
- **Test strategy:** PTY tests use isolated `ActivePids` instances to prevent cross-test kills; run with `--test-threads=1`.

### Git CLI

- **Purpose:** Worktree creation/removal, diff/status capture for review packets.
- **Configuration:** Requires `git` on PATH.
- **Outbound operations:** `git worktree add`, `git worktree remove`, `git rev-parse`, `git diff`, `git status`, `git diff --name-only`.
- **Implementation:** `task/complete.rs` (`git_command`, `run_git`, `run_git_stdout`).
- **Error handling:** Git failures become task diagnostics or launch errors; non-zero exit codes are propagated as strings.
- **Test strategy:** Dog-food tests use real git repos; fixture tests use mocked paths where possible.

### Claude Code CLI (via Host Runner)

- **Purpose:** Executing Claude-powered tasks in an authentic interactive environment.
- **Protocol:** Custom JSON-over-Unix-socket protocol (version 2). Request frame → host runner → PTY spawn → ANSI transcript capture → response frame.
- **Configuration:** `AGENT_BRIDGE_CLAUDE_HOST_SOCKET` pointing to a running `claude-host-runner` socket.
- **Credential handling:** Claude Code obtains its own API keys via the host shell (Keychain, env, or cloud creds); Agent Bridge never holds Anthropic credentials.
- **Error handling:** Failure categories (`ClaudeAuthError`, `ClaudeRateLimit`, etc.) are mapped from host-runner error frames.
- **Test strategy:** Fake host runner in integration tests; no real Claude subscription required.

### Provider CLIs (Codex, Cursor, Kimi, Antigravity)

- **Purpose:** Direct execution of non-Claude provider agents.
- **Protocol:** Fork/exec with filtered environment and optionally piped stdin.
- **Configuration:** Binaries must be on PATH; no env vars required by Agent Bridge.
- **Error handling:** Denial detection via stderr heuristics (e.g., Codex sandbox rejection); output acceptability via stdout parsers.
- **Test strategy:** Deterministic fake provider scripts in `tests/fixtures/` simulate success/failure/denial without paid API access.

## Integration Dependency Graph

```mermaid
architecture-beta
    group agent_bridge(cloud)[Agent Bridge MCP]
        service runtime(server)[Runtime] in agent_bridge
        service server_router(server)[Server Router] in agent_bridge
        service task_manager(server)[Task Manager] in agent_bridge
        service provider_cmds(server)[Provider Commands] in agent_bridge

    group host_system(server)[Host System]
        service posix(kernel)[OS / POSIX] in host_system
        service git(cli)[Git CLI] in host_system
        service fs(disk)[Filesystem] in host_system

    group providers(cloud)[Provider Agents]
        service claude(cli)[Claude Code] in providers
        service codex(cli)[Codex CLI] in providers
        service cursor(cli)[Cursor Agent] in providers
        service kimi(cli)[Kimi/Pi CLI] in providers
        service agy(cli)[Antigravity CLI] in providers

    runtime:R --> L:posix
    server_router:R --> L:posix
    task_manager:R --> L:posix
    task_manager:T --> B:fs
    task_manager:R --> L:git
    provider_cmds:R --> L:claude
    provider_cmds:R --> L:codex
    provider_cmds:R --> L:cursor
    provider_cmds:R --> L:kimi
    provider_cmds:R --> L:agy
```
