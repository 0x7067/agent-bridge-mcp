## Delta Headers

```yaml
delta_of: task-lifecycle-resilience
change: fix-timeout-misclassification
author: agent
status: accepted
```

## MODIFIED Requirements

### Requirement: Timeout intent takes precedence over late exit codes
When a task exceeds its allocated timeout, the resulting status SHALL be `Failed` with `FailureCategory::TimedOut`, regardless of the provider process's eventual exit code.

**Previous Behavior:**
The provider's exit code could overwrite a timeout-induced failure if the process exited with code 0 after the timeout signal was delivered.

**Updated Behavior:**
The completion classifier evaluates the `timed_out` flag before inspecting the exit code. Once `timed_out == true`, the task is unconditionally classified as `TimedOut`.

#### Scenario: Late success after timeout
- **WHEN** a slow provider receives SIGTERM at T+30s
- **AND** it flushes buffers and exits 0 at T+30.5s
- **THEN** the task status is `TimedOut` with a diagnostic noting the timeout budget
- **AND** the previous behavior would have incorrectly recorded `Succeeded`.

### Requirement: Pre-deadline exits are protected from scheduler jitter
If a provider process exits cleanly before the timeout budget elapses, the task SHALL be classified as `Succeeded` even if the timeout future technically resolves first due to scheduling jitter.

**Mechanism:**
Capture the `Instant` at which the timeout budget is exhausted. If the child-exit future resolves strictly before that instant, ignore the subsequently arriving timeout notification.

#### Scenario: Fast success colliding with timeout future
- **WHEN** a provider finishes at T+29.999s
- **AND** the kernel schedules the timeout thread first, waking the timeout future
- **THEN** the pre-deadline exit timestamp protects the legitimate `Succeeded` classification
- **AND** the previous behavior could have incorrectly yielded `TimedOut`.

## ACCEPTANCE CRITERIA

- Unit tests simulate late exit(0) and assert `TimedOut` status.
- Unit tests simulate near-deadline clean exit and assert `Succeeded` status.
- No existing success-path tests are broken by the new ordering.
