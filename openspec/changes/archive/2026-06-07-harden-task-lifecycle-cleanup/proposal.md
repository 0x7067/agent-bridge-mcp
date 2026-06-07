## Why

Agent Bridge spawns and supervises long-running provider processes inside managed git worktrees, but several lifecycle paths leak resources on partial failure. Worktree and process cleanup is non-transactional, the host runner threads a process-id registry that is never populated, the post-SIGKILL wait has no upper bound, the panic hook does not reap children, and `spawn` applies no backpressure. Each of these turns a transient failure into an orphaned worktree, a zombie process, or an exhausted host. A delegation server that runs unattended must guarantee that every spawned resource is either tracked or reclaimed.

## What Changes

- Make worktree removal in `remove` transactional: order registry persistence and directory removal so a failure at any step leaves a reclaimable record rather than a silent orphan.
- Auto-clean a managed worktree when launch fails after the worktree was created, so a failed spawn does not require a manual `agent_remove`.
- Populate the host runner process-id registry on spawn so the existing shutdown signal can actually terminate active child processes; remove the dead parameter if tracking is not wired.
- Add a bounded wait after SIGKILL and return best-effort status with a diagnostic when a child cannot be reaped within the bound.
- Reap or signal active children from the panic hook before abort.
- Add a configurable maximum-concurrent-task limit with a clear "too many active tasks" error and next-action guidance.
- Add a startup reconciliation sweep that detects worktrees and registry records orphaned by a prior crash and reports or reclaims them.

## Capabilities

### New Capabilities

- `task-lifecycle-resilience`: Covers transactional worktree cleanup, launch-failure cleanup, child-process tracking and reaping, bounded forced-termination, panic-time shutdown, spawn backpressure, and crash-orphan reconciliation.

### Modified Capabilities

- `rust-single-binary-mcp`: The Rust MCP surface must expose the concurrency-limit error and the reconciliation results through existing tool and doctor outputs.

## Impact

- Affected code: task spawn/stop/remove/complete and child supervision in `crates/agent-bridge-mcp/src/task.rs`, host runner child tracking in `crates/agent-bridge-mcp/src/claude_host.rs`, panic hook in `crates/agent-bridge-mcp/src/runtime.rs`, and reconciliation/limit surfacing in `crates/agent-bridge-mcp/src/server.rs`.
- Affected APIs: additive concurrency-limit error and additive doctor reconciliation fields; no tool renames.
- Affected docs/specs: README safety/lifecycle notes and doctor contract.
- Dependencies: no new third-party dependency is expected.
