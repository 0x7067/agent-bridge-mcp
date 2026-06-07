---
status: accepted
date: 2025-04-??
---

# ADR-0002: Harden Task Lifecycle and Extract Provider Adapter Abstraction

## Context

As provider coverage expanded beyond simple fork/exec (Claude, Codex, Cursor, Kimi, Antigravity), the ad-hoc command-building logic in `server.rs` grew unwieldy. Each provider had subtly different launch strategies, output expectations, denial signatures, and environment requirements. The task lifecycle (spawn → supervise → collect → classify → review) lacked uniform error taxonomy, causing inconsistent failure reporting. Crash recovery also leaked orphaned worktrees and zombie PIDs.

## Decision

We decided to introduce a typed `FailureCategory` enum covering all recoverable and fatal error classes, and extract a provider adapter trait surfaced through `provider.rs`. Each adapter defines:

- Supported modes, profiles, and capabilities
- Command-line construction and environment filtering
- Stderr denial detection heuristics
- Output acceptability checks
- Cadence hints for observation polling

The task lifecycle was hardened with:

- Explicit status-transition validation (`transition_status`)
- PID registration/unregistration in a global `ActivePids` registry
- Graceful SIGTERM → SIGKILL escalation with configurable timeouts
- Startup-time reconciliation of orphaned `Queued`/`Running` records
- Auto-removal of abandoned worktrees on server restart

### Considered Alternatives

#### Inline provider switches everywhere

- Good, because immediate locality.
- Bad, because explosion of `match provider` arms across spawn, supervise, diagnose, and observe codepaths.

#### Separate microservices per provider

- Good, because strong isolation.
- Bad, because this is a desktop stdio server; IPC overhead defeats the purpose.

## Consequences

### Positive

- Adding a sixth provider requires only a new adapter impl in `provider.rs`.
- Consistent failure taxonomy lets `doctor` and `retry_policy` act uniformly.
- Orphaned process/worktree leaks eliminated on server restart.

### Negative

- Global `ACTIVE_PIDS` mutex adds contention; acceptable given desktop-scale concurrency.

### Neutral

- `TaskManagerHandle` is a singleton `OnceCell`; tests must tolerate shared state.

## Evidence

- **Commit(s):** `309ab11`, `8aa4a9d`
- **Key files changed:** `src/provider.rs`, `src/task.rs`, `src/task/supervision.rs`, `src/server/diagnostics.rs`, `src/runtime.rs`
- **Blast radius:** 5 files, ~2500 lines changed.
- **Timeline:** Delivered in two stacked PRs over ~1 week.
