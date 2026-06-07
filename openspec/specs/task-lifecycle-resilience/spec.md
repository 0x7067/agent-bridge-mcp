# task-lifecycle-resilience Specification

## Purpose
TBD - created by archiving change harden-task-lifecycle-cleanup. Update Purpose after archive.
## Requirements
### Requirement: Worktree cleanup is transactional
The task manager SHALL order worktree removal so that a failure at any single step leaves a reclaimable record rather than a silently orphaned worktree.

#### Scenario: Directory removal fails
- **WHEN** removing a task whose managed worktree directory removal fails
- **THEN** the failure is recorded on the task record
- **AND** the task is discoverable by the reconciliation sweep for later reclaim.

#### Scenario: Persistence fails after git removal
- **WHEN** git worktree removal succeeds but registry persistence fails
- **THEN** the operation surfaces an error
- **AND** no worktree is left untracked after the next reconciliation sweep.

### Requirement: Launch failure cleans its worktree
The task manager SHALL clean or queue for reclaim a managed worktree that was created when the subsequent provider launch fails.

#### Scenario: Provider launch fails after worktree creation
- **WHEN** a worktree is created and the provider launch then fails
- **THEN** the worktree is removed or queued for the reconciliation sweep
- **AND** no manual removal call is required to reclaim it.

### Requirement: Active child processes are tracked and reaped
The host runner SHALL track active child process ids and terminate them on shutdown.

#### Scenario: Shutdown terminates children
- **WHEN** the host runner receives a shutdown signal with active children
- **THEN** each tracked child receives SIGTERM and, if still alive after the grace period, SIGKILL.

#### Scenario: Forced termination is bounded
- **WHEN** a child does not exit after SIGKILL within the bounded wait
- **THEN** the manager returns best-effort status
- **AND** logs an orphaned-process diagnostic instead of waiting indefinitely.

### Requirement: Panic-time cleanup
The panic hook SHALL attempt to terminate active children before aborting.

#### Scenario: Panic during active tasks
- **WHEN** the server panics with active child processes
- **THEN** the panic hook signals those children before the process aborts.

### Requirement: Spawn backpressure
The task manager SHALL enforce a configurable maximum number of concurrent active tasks.

#### Scenario: Limit exceeded
- **WHEN** a spawn would exceed the configured concurrency limit
- **THEN** the spawn is rejected with a structured "too many active tasks" error
- **AND** the error includes next-action guidance to wait for or stop existing tasks.

### Requirement: Crash-orphan reconciliation
The server SHALL reconcile worktrees and registry records orphaned by a prior crash on startup.

#### Scenario: Orphaned worktree on startup
- **WHEN** the server starts and finds a managed worktree without a live owning task
- **THEN** the orphan is reported through a `doctor` section
- **AND** is reclaimed or flagged according to the configured policy.

