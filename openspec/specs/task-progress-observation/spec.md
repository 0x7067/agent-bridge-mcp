# task-progress-observation Specification

## Purpose
TBD - created by archiving change improve-task-progress-observation. Update Purpose after archive.
## Requirements
### Requirement: Agents expose progress observation metadata
The system SHALL expose progress metadata for running provider agents so callers can distinguish healthy silence, recent output, timeout risk, and finalization state without parsing raw logs.

#### Scenario: Running task with no provider output
- **WHEN** a caller inspects a running task that has emitted no stdout or stderr
- **THEN** the response includes progress metadata with elapsed time, last lifecycle event time, missing last output time, expected output cadence, recommended next check timing, effective silent budget, and stall risk.
- **AND** the response does not classify the task as failed solely because no output has been emitted.

#### Scenario: Running task after provider output
- **WHEN** a caller inspects a running task after stdout, stderr, or structured transcript output has been recorded
- **THEN** the response includes last output timing, transcript cursor information, and a lower or updated stall risk derived from the recent activity.

#### Scenario: Final task progress metadata
- **WHEN** a caller inspects a final task
- **THEN** progress metadata reports that no further polling is needed and points callers to final result inspection.

### Requirement: Agent observation supports bounded long polling
The system SHALL provide a bounded request/response observation surface for waiting on agent lifecycle or transcript changes without requiring clients to poll in a tight loop.

#### Scenario: Observe returns new events
- **WHEN** a caller invokes `agent_observe` with a task id, cursor, limit, and timeout
- **THEN** the response waits up to the requested timeout for new lifecycle or transcript events
- **AND** returns the current task summary, events since the cursor, next cursor, progress metadata, and observe-call timeout status.

#### Scenario: Observe times out without agent failure
- **WHEN** a caller invokes `agent_observe` and no new task events arrive before the observe timeout
- **THEN** the response reports the observe call as timed out
- **AND** preserves the agent's actual running status unless the provider process itself reached a final state.

#### Scenario: Observe is bounded
- **WHEN** a caller invokes `agent_observe` with a very large timeout or limit
- **THEN** the server clamps the request to documented maximums and never returns unbounded logs, diffs, or transcript payloads.

#### Scenario: Observe sees finalization
- **WHEN** a caller invokes `agent_observe` while a task reaches a final state
- **THEN** the response returns the final task status, final progress metadata, any available final lifecycle events, and does not require a separate polling loop to discover finalization.

### Requirement: Progress recommendations avoid premature fallback
The system SHALL recommend wait, observe, inspect, stop, or fallback actions based on provider output cadence, elapsed time, task timeout, and available transcript evidence.

#### Scenario: Silent final-output provider within expected budget
- **WHEN** a provider task is running, has no provider output, and is still within the provider's recommended silent-output budget
- **THEN** progress recommendations prefer another bounded observe or wait action
- **AND** stop or fallback is not the primary recommendation.

#### Scenario: Progress heuristic is deterministic
- **WHEN** progress metadata is returned
- **THEN** `stallRisk` is one of `none`, `low`, `medium`, or `high`
- **AND** it is derived deterministically from provider cadence metadata, elapsed time, silent duration, configured timeout, and finality.

#### Scenario: Agent exceeds expected silence budget
- **WHEN** a provider agent remains silent beyond its expected output budget or near the configured task timeout
- **THEN** progress recommendations include log/transcript inspection and may mark stop or fallback as available with a reason.

#### Scenario: Provider timeout remains authoritative
- **WHEN** a provider task reaches its configured task timeout
- **THEN** the task finalizes with timeout semantics independently from any observe-call timeout.

