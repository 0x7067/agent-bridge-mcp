## Context

Findings from a code audit (file:line cited in `tasks.md`) identified five resource-leak paths in the task lifecycle. All are low-frequency but high-consequence: each leaves persistent state (a worktree, a process, a corrupted registry) that survives the failing request. The server already persists a task registry and retains managed worktrees for inspection, so the fix is to make every transition either complete or reclaimable, never silently partial.

## Goals / Non-Goals

- Goals: no orphaned worktrees or processes after any single-step failure; bounded forced termination; backpressure under load; recovery of crash orphans on restart.
- Non-Goals: distributed coordination, multi-host supervision, or changing the spawn/observe/result/remove tool surface.

## Decisions

- **Transactional ordering over rollback.** True rollback of a deleted git worktree is impossible, so order operations so the irreversible step (directory removal) is last and is preceded by a persisted "removing" marker. A crash between steps is recovered by the startup sweep rather than by in-process rollback.
- **Reconciliation is the safety net.** Rather than guarantee in-process cleanup for every panic and kill path, persist enough state that a startup sweep can always find and reclaim orphans. This keeps the hot path simple and makes the guarantee crash-robust.
- **Concurrency limit is configurable, default conservative.** A fixed semaphore-style limit on active tasks; surfaced as a structured error with next actions, consistent with the existing self-guiding response style.
- **Bounded kill wait, never unbounded.** After SIGKILL, wait at most a short fixed bound, then return best-effort status. A process the OS cannot reap is reported, not waited on forever.

## Risks / Trade-offs

- The startup sweep could reclaim a worktree a user still wanted; mitigate by reporting before reclaiming and gating destructive reclaim behind policy/flag.
- A concurrency limit can reject legitimate bursts; mitigate with a clear error and guidance, and make the limit configurable.

## Migration Plan

Additive and internal. No client-visible tool changes beyond a new error variant and new doctor fields. No backwards-compatibility shim is required (per project direction); the registry format gains a reclaim marker field read defensively.
