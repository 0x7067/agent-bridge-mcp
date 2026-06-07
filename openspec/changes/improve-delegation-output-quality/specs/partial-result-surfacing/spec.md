# partial-result-surfing Specification

## Purpose
Surface incomplete but meaningful provider output when a task crashes or times out, guiding the caller toward continuation or salvage.

## ADDED Requirements

### Requirement: Partial results populate the result payload
The system SHALL include a `partialResults` array in `agent_result` when the task is final, `final_result_detected` is false, and `partial_result_detected` is true.

#### Scenario: Partial result present
- **WHEN** a task times out after producing substantial provider output
- **THEN** `agent_result` includes `partialResults` containing the last N meaningful transcript events.

#### Scenario: No partial result
- **WHEN** a task fails before producing any recognizable output
- **THEN** `partialResults` is omitted or empty.

### Requirement: Next actions suggest continuation for partial results
The system SHALL update the `next` action list to recommend salvaging or rerunning from partial state when `partialResults` is nonempty.

#### Scenario: Partial result guidance
- **WHEN** a caller inspects a failed task with partial results
- **THEN** the `next` list includes an action suggesting continuation or narrowing the prompt rather than discarding the work.

### Requirement: Partial result scan is bounded
The system SHALL scan no more than the last 1,024 transcript lines when extracting partial results.

#### Scenario: Large transcript truncation
- **WHEN** a transcript contains tens of thousands of lines
- **THEN** the partial result scan examines only the tail to bound computation during finalization.
