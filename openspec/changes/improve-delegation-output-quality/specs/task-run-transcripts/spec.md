# task-run-transcripts Delta Specification

## Purpose
Expand the transcript inspection surface to expose partial results when a task terminates before delivering a complete provider response.

## ADDED Requirements

### Requirement: Transcript inspection includes partial result events
The system SHALL augment the `agent_result` transcript section to include `partialResult` markers when the underlying transcript contains provider output that was not finalized.

#### Scenario: Transcript read exposes partial markers
- **WHEN** a caller requests `agent_result` with `sections: ["transcript"]` for a task that timed out after generating useful output
- **THEN** transcript events flagged as `partialResult: true` indicate which lines constitute recoverable progress.

#### Scenario: Partial markers do not spoof final results
- **WHEN** transcript parsing encounters a genuine `provider_result` event
- **THEN** it is marked `finalResult: true` and never commingles with `partialResult` events.

## MODIFIED Requirements

### Requirement: Transcripts support final and partial result detection
The system SHALL use transcript evidence to detect provider final-result and partial-result signals without treating provider prose as verification.

#### Scenario: Final result detected before process stop
- **WHEN** transcript events show that a provider emitted a complete final result before the process is stopped or times out
- **THEN** the task result exposes diagnostic metadata indicating that a final result was detected.

#### Scenario: Partial result detected
- **WHEN** transcript events show useful provider progress but no complete final result
- **THEN** the task result exposes diagnostic metadata indicating that partial provider output is available for inspection.
- **AND** the `agent_result` payload includes `partialResults` derived from the transcript tail.

#### Scenario: Absent result detection
- **WHEN** a task produces neither final nor partial result evidence
- **THEN** the task result omits both `finalResultDetected` and `partialResultDetected` affirmatively.
