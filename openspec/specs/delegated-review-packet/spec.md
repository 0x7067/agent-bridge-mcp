# delegated-review-packet Specification

## Purpose
Define the delegated task review-packet summary returned by `task_result`, including how it presents existing result evidence, recommended caller actions, and verification boundaries without interpreting provider output as proof.
## Requirements
### Requirement: Task results include a delegated review packet
The system SHALL include an additive `reviewPacket` object in `task_result` responses that summarizes existing task result evidence for caller inspection.

#### Scenario: Successful task with no repository changes
- **WHEN** a caller reads `task_result` for a successful task whose git status and changed files are empty
- **THEN** the response includes `reviewPacket.status`, `reviewPacket.isFinal`, `reviewPacket.hasChanges: false`, `reviewPacket.changedFiles: []`, truncation flags, exit metadata, and recommended actions that tell the caller to inspect provider output and run relevant verification before claiming completion.

#### Scenario: Final task with repository changes
- **WHEN** a caller reads `task_result` for a final task with changed files or non-empty git status
- **THEN** `reviewPacket.hasChanges` is true, `reviewPacket.changedFiles` mirrors the existing `changedFiles` field, and recommended actions include inspecting the diff before verification and cleanup.

#### Scenario: Failed task result
- **WHEN** a caller reads `task_result` for a failed task
- **THEN** the review packet includes `errorType`, exit metadata, diagnostic data when available, and recommended actions that point the caller to logs, diagnostics, and rerun or manual recovery decisions.

#### Scenario: Running task result
- **WHEN** a caller reads `task_result` for a task that is not final
- **THEN** the review packet includes `isFinal: false` and recommended actions that point the caller to bounded waits, incremental logs, status inspection, or stopping the task if it is no longer useful.

#### Scenario: Managed worktree result
- **WHEN** a caller reads `task_result` for a task that used managed worktree isolation
- **THEN** recommended actions include calling `task_remove` only after the managed worktree result has been inspected.

### Requirement: Review packets remain derived evidence
The system SHALL keep review packets as summaries of existing result fields rather than provider-output interpretation or verification claims.

#### Scenario: Review packet generation
- **WHEN** a review packet is generated
- **THEN** it does not parse provider prose, does not claim tests passed, and does not remove or change any existing `task_result` fields.

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

### Requirement: Review packets guide Codex denial recovery
The system SHALL include recovery guidance in `reviewPacket` for failed Codex tasks whose diagnostics indicate sandbox, approval, or out-of-workspace patch denials.

#### Scenario: Codex denial review packet
- **WHEN** a caller reads `task_result` for a failed Codex task with sandbox or approval denial diagnostics
- **THEN** `reviewPacket.recommendedActions` directs the caller to inspect stderr/logs, verify `cwd` and workspace policy, narrow the prompt or isolation strategy, and avoid claiming completion

#### Scenario: Review packet avoids unsafe automatic retry advice
- **WHEN** a Codex denial review packet is generated
- **THEN** it does not recommend silently relaxing sandbox permissions or blindly retrying without inspecting the diagnostic evidence

#### Scenario: Review packet remains derived evidence
- **WHEN** a Codex denial review packet is generated
- **THEN** it summarizes available status, diagnostics, logs, and exit metadata without parsing provider prose as proof of project correctness

