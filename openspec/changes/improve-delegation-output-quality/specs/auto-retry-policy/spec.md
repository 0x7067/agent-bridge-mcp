# auto-retry-policy Specification

## Purpose
Enable bounded, automatic retry for transient provider failures so callers do not need to reconstruct and resubmit `agent_spawn` requests manually.

## ADDED Requirements

### Requirement: Agent spawn accepts optional retry policy
The system SHALL accept an optional `retryPolicy` object in `agent_spawn` arguments containing `maxRetries` (uint, default 0) and `backoffMs` (uint, default 1000).

#### Scenario: Spawn without retry policy
- **WHEN** a caller invokes `agent_spawn` without `retryPolicy`
- **THEN** the task proceeds with `maxRetries: 0` and no retry behavior.

#### Scenario: Spawn with retry policy
- **WHEN** a caller invokes `agent_spawn` with `retryPolicy: { "maxRetries": 2, "backoffMs": 2000 }`
- **THEN** the task record stores the policy and the actor evaluates it upon transient failure.

### Requirement: Actor retries transient failures only
The system SHALL retry only when the completion carries a failure category classified as transient: `ProviderTimeout`, `ProviderStartError`, or `HostRunnerUnavailable`.

#### Scenario: Transient failure triggers retry
- **WHEN** a task times out and `maxRetries` is greater than 0
- **THEN** the actor schedules a respawn after the jittered backoff delay.

#### Scenario: Permanent failure does not retry
- **WHEN** a task fails with `ProviderOutputError` or `CodexSandboxDenied`
- **THEN** the actor finalizes the task without retry regardless of `maxRetries`.

### Requirement: Retry budget is decremented and persisted
The system SHALL decrement the remaining retry count and append a `retry_attempt` event to the transcript before each respawn.

#### Scenario: Budget exhaustion
- **WHEN** a task has exhausted its retry budget
- **THEN** the actor finalizes with the last received failure category.

### Requirement: Backoff is jittered and capped
The system SHALL compute the next delay as `min(backoffMs * 2^attempt, 30000)` plus a random jitter of up to 25%.

#### Scenario: Jittered backoff
- **WHEN** a retry is scheduled for the second attempt with `backoffMs: 1000`
- **THEN** the actual delay is between `2000` and `2500` milliseconds.

#### Scenario: Backoff cap
- **WHEN** the computed delay exceeds 30 seconds
- **THEN** the delay clamps to 30 seconds.
