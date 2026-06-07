## Why

Under certain scheduling conditions, a timed-out task could be misclassified as successful. If a provider process exited with code 0 shortly after the timeout deadline fired, the supervisor's completion logic preferred the child's exit status over the timer's intent, recording `Succeeded` when the task had actually breached its SLA. This caused silent data loss in downstream review packets and broke the caller's expectation that a timeout reliably produces `Failed`.

## What Changes

- Adjust `task/supervision.rs` and `task/complete.rs` so that the timeout branch takes priority over the provider-exited-cleanly branch.
- Prevent two distinct flaky classes:
  1. Premature stop classification when a slow provider exits with code 0 after the timeout signal was already dispatched.
  2. Successful misclassification when a fast provider finishes moments before the timeout fires but the scheduler delivers the timeout notification first.
- Emit a `TimedOut` diagnostic category with a clear explanation instead of swallowing the incident into a success bucket.

## Capabilities

### New Capabilities
*(No new capabilities.)*

### Modified Capabilities
- `task-lifecycle-resilience`: Updates the completion-order guarantee so timeout intent is authoritative over late exit codes.

## Impact

- Behavioral change in `task/supervision.rs` and `task/complete.rs`.
- No public API or schema changes; callers simply observe fewer incorrect `Succeeded` statuses.
- Risk: slightly increased `Failed` rate for tasks that were previously silently blessed.
