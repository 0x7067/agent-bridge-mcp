## ADDED Requirements

### Requirement: Review packets summarize transcript availability
The system SHALL include transcript availability and transcript-derived result evidence in delegated review packets without turning provider prose into verification claims.

#### Scenario: Transcript available
- **WHEN** a caller reads `task_result` for a task with transcript events
- **THEN** the review packet indicates that transcript evidence is available and recommends inspecting the transcript when provider behavior or final-state classification is unclear.

#### Scenario: Final result detected from transcript
- **WHEN** transcript analysis detects a provider final result
- **THEN** the review packet reports that final-result evidence exists and still recommends caller-side verification before claiming work complete.

#### Scenario: Partial result detected from transcript
- **WHEN** transcript analysis detects partial provider progress but no complete final result
- **THEN** the review packet reports partial-result evidence and recommends inspecting transcript and logs before deciding whether to rerun, continue manually, or discard.
