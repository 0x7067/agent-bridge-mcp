## ADDED Requirements

### Requirement: Tasks persist normalized run transcripts
The system SHALL persist a normalized transcript artifact for each spawned provider task while preserving raw stdout and stderr logs.

#### Scenario: Transcript artifact created
- **WHEN** a caller spawns a provider task
- **THEN** the task directory contains a transcript artifact that records timestamped task run events derived from stdout, stderr, lifecycle transitions, and provider-specific structured output where available.

#### Scenario: Raw logs remain available
- **WHEN** a caller reads task logs or task results for a task with a transcript
- **THEN** existing stdout and stderr log inspection remains available independently from the transcript artifact.

### Requirement: Transcript collection is best-effort and non-fatal
The system SHALL treat transcript parsing as best-effort metadata collection rather than task success criteria.

#### Scenario: Provider output is not parseable
- **WHEN** provider output cannot be parsed into a structured provider event
- **THEN** the system records a raw transcript event and does not fail the task solely because transcript parsing failed.

#### Scenario: Transcript write fails
- **WHEN** transcript event persistence fails but raw logs and task lifecycle can still be recorded
- **THEN** the task finalizes according to provider lifecycle behavior and exposes transcript diagnostics rather than replacing the provider result.

### Requirement: Transcripts are exposed through a bounded inspection surface
The system SHALL expose task transcripts through a bounded public MCP inspection surface with cursor or limit controls.

#### Scenario: Reading transcript events
- **WHEN** a caller requests a task transcript
- **THEN** the response returns transcript events up to the requested or configured cap and includes cursor metadata for subsequent reads.

#### Scenario: Reading a missing transcript
- **WHEN** a caller requests a transcript for a task that predates transcript collection or has no transcript artifact
- **THEN** the response clearly reports transcript unavailability without hiding existing task logs or result metadata.

### Requirement: Transcript exposure redacts sensitive content
The system SHALL redact sensitive prompt bodies, configured secrets, and provider environment values before writing transcript events and before exposing transcript events through public MCP responses.

#### Scenario: Prompt redaction
- **WHEN** a transcript event contains the rendered prompt or original task prompt
- **THEN** the public transcript response redacts that prompt content while preserving enough event metadata for debugging.

#### Scenario: Secret redaction
- **WHEN** a transcript event contains a value from the provider environment redaction set
- **THEN** the public transcript response redacts that value.

#### Scenario: Stored transcript redaction
- **WHEN** a transcript event is persisted to the task transcript artifact
- **THEN** the stored event redacts known prompt bodies and configured secrets rather than relying only on read-time redaction.

### Requirement: Transcripts support final and partial result detection
The system SHALL use transcript evidence to detect provider final-result and partial-result signals without treating provider prose as verification.

#### Scenario: Final result detected before process stop
- **WHEN** transcript events show that a provider emitted a complete final result before the process is stopped or times out
- **THEN** the task result exposes diagnostic metadata indicating that a final result was detected.

#### Scenario: Partial result detected
- **WHEN** transcript events show useful provider progress but no complete final result
- **THEN** the task result exposes diagnostic metadata indicating that partial provider output is available for inspection.

### Requirement: Transcript parsing is fixture-backed per provider
The system SHALL verify transcript parsing and result detection with provider-specific fixtures for supported provider output formats.

#### Scenario: Provider fixture coverage
- **WHEN** transcript parsing is implemented for a provider output format
- **THEN** tests cover that format with representative stdout/stderr fixtures and expected transcript events.

#### Scenario: Result marker validation
- **WHEN** transcript parsing detects final-result or partial-result evidence
- **THEN** tests prove the marker validation does not treat arbitrary result-like text as a complete provider final result.
