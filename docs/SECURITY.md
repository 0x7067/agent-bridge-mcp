# Security Patterns

**Last Updated:** 2026-06-16

## User Roles

This system does not implement RBAC, CASL, or role-based access control. It is a single-user desktop stdio server. The security model assumes:

- The MCP host process and Agent Bridge share the same OS user identity.
- The human operator trusts the primary agent and the provider CLIs they install.
- Provider API keys live in the operator's ambient environment (shell env, provider config files, OS keychain), not in Agent Bridge state.

## Authentication

- **Method:** Implicit OS-subprocess trust. The MCP client spawns Agent Bridge as a child process; authentication is inherited via UID/GID and filesystem permissions.
- **Session storage:** None.
- **Entry point:** `main.rs` → `runtime::main_entry()`.

## Authorization Architecture

Because there are no multiple principals, authorization reduces to **environmental confinement** and **input sanitization**:

### Workspace Confinement

- **What:** `AGENT_BRIDGE_WORKSPACES` declares a colon-separated list of allowed directory trees.
- **Where enforced:** `task/spawn.rs` (`safe_cwd`)
- **Behavior:** Any `cwd` outside these roots is rejected with an error. Paths containing `..` are rejected outright.
- **Unblocked profile:** `profile: "unblocked"` may add provider-specific permission-bypass flags, but only after this workspace check succeeds. It is not a substitute for workspace confinement and does not broaden configured roots.

### Input Sanitization

- **Deny unknown fields:** All tool inputs declare `#[serde(deny_unknown_fields)]`, preventing hallucinated parameters from reaching logic.
- **Prompt capping:** `MAX_PROMPT_BYTES` (100 KiB) rejects oversized prompts.
- **Argument clamping:** Numeric values (timeouts, limits, bytes) are clamped to sensible ranges.

### Secret Hygiene

- **Redaction:** `diagnostic_redactions()` scans the provider environment for keys containing `KEY`, `TOKEN`, or `SECRET` and scrubs their values from transcripts and diagnostics.
- **Argv avoidance:** Claude provider injects prompt text via PTY keystrokes, never via command-line arguments.
- **Env clearing:** Child processes are spawned with `env_clear()`, then repopulated with an explicit provider allowlist.

## Data Auditing

No formal audit/logging framework. Informal audit traces exist in:

- `transcript.jsonl` — immutable-sequence (append-only) of every provider event
- `registry.json` — atomic snapshots of all task metadata
- `stdout.log` / `stderr.log` — capped captures of provider output

These files reside in the operator's private state directory (`~/.agent-bridge-mcp/state` by default) and inherit OS filesystem permissions.

## Multi-Tenant Isolation

Not applicable. Single-tenant. However, the **Worktree Isolation** pattern achieves a degree of spatial separation:

- **Strategy:** Each mutable task may receive a disposable git worktree.
- **Creation:** `git worktree add -b agent-bridge/...` in a subdirectory of `STATE_DIR/worktrees`.
- **Cleanup:** Removed during managed cleanup after inspection, or reclaimed automatically if the server crashed while the task was running.
- **Boundary:** The worktree is a sibling checkout of the same repo; it does not protect against malicious providers that escape the working directory via absolute paths.

## Threat Model Notes

| Concern | Mitigation | Limitation |
|---------|------------|------------|
| Malicious provider escapes cwd | Workspace confinement + worktree isolation | Not a sandbox; determined code can traverse the filesystem |
| Unblocked provider overreach | Explicit opt-in profile + pre-launch workspace validation + smoke probe for workspace write/read/delete reach | Provider bypass flags may still grant broader host access than Agent Bridge can enforce |
| Sensitive env leaked in logs | Keyword-based redaction in transcripts | Greedy heuristic may miss novel secret names |
| Provider API key theft | Keys never held by Agent Bridge | Compromised host machine still exposes ambient keys |
| Long-running orphan processes | Active PID registry + panic-hook SIGTERM | Race windows exist between crash and hook execution |
| Prompt injection via MCP | `deny_unknown_fields` + capped lengths | Does not defend against semantic injection in prompt content |
