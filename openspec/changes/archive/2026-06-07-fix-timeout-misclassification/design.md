## Context

The `wait_for_child` routine in `task/supervision.rs` drives a `tokio::select!` among three futures:
1. Child process exit.
2. STDERR readiness (for Codex denial detection).
3. Timeout expiration.

Previously, the completion handler evaluated whichever future resolved first. If the timeout fired and armed a `timed_out = true` flag, but the child process exited with code 0 before the subsequent classification logic ran, the success path took precedence. Conversely, if the child finished legitimately just microseconds before the timeout, the scheduler could still deliver the timeout notification first, causing a false `Failed`.

## Goals / Non-Goals

**Goals:**
- Make timeout intent definitive when the timeout budget has elapsed.
- Avoid penalizing tasks that finish legitimately before the deadline.
- Produce a deterministic outcome independent of kernel scheduling jitter.

**Non-Goals:**
- Extending or shortening the timeout policy itself.
- Changing the retry/backoff behavior (handled in `improve-delegation-output-quality`).
- Altering the public tool schema or MCP response envelope.

## Decisions

### Priority of intents over observations
Adopted a simple precedence rule: if the timeout budget expired, the task is `TimedOut` regardless of the eventual exit code. If the budget had not expired when the child exited normally, the task is `Succeeded`. This collapses the race window into a single, comprehensible predicate.

### Explicit `TimedOut` diagnostic category
Added a dedicated `FailureCategory::TimedOut` variant mapped to the `"timed-out"` kebab-string. This lets callers and the auto-retry policy recognize the failure as transient by default.

### No additional synchronization primitives
Rejected wrapping the exit and timeout futures in a mutex or atomic state machine. The added complexity outweighed the benefit; reordering the conditional branches in the completion classifier achieved the same determinism with zero extra allocations.

## Risks / Trade-offs

- [False positive] Legitimate successes that happen exactly at the deadline may occasionally be classified as timeouts if the timeout future wins the select. → Accepted: the alternative (false negatives) is worse for SLAs.
- [Metric shift] Dashboards tracking success rates may dip slightly. → Communicate the fix as a correction, not a regression.

## Migration Plan

Single focused commit (`6680059`). Deployed to `main` as a hotfix. No rollbacks anticipated; the prior behavior was acknowledged as buggy.

## Open Questions

- Should we tighten the grace margin (e.g., treat exits within ±50ms of the deadline as indeterminate)? Deferred: simpler is safer.
