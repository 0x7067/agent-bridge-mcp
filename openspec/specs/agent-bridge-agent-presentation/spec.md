# agent-bridge-agent-presentation Specification

## Purpose
TBD - created by archiving change make-agent-bridge-agents-native-feeling. Update Purpose after archive.
## Requirements
### Requirement: Tasks expose client-renderable presentation summaries
The system SHALL expose a compact `presentation` summary for Agent Bridge tasks through the existing `agent_*` lifecycle surface so MCP clients can render them as native-feeling agents without parsing raw logs, full task results, or provider prose.

#### Scenario: Active task summary
- **WHEN** a client requests the presentation summary for a running Agent Bridge task
- **THEN** the response includes the task identifier, display title, provider, mode, lifecycle phase, status tone, workspace path, created and updated timestamps, duration when available, and whether a final result is available.

#### Scenario: Final task summary
- **WHEN** a client requests the presentation summary for a final Agent Bridge task
- **THEN** the response includes final status, completion timestamp, duration, error type when present, changed-file count, transcript availability, whether final or partial provider result evidence was detected, and `verificationStatus: "not_verified"`.

#### Scenario: Stale task summary
- **WHEN** a client requests the presentation summary for a task marked `failed_stale`
- **THEN** the response uses a final presentation phase, includes the stale error type, and exposes result-inspection actions without implying the task can be resumed.

#### Scenario: Summary avoids raw payloads
- **WHEN** a client requests a compact presentation summary
- **THEN** the response does not include raw stdout, raw stderr, full git diffs, or full transcript events.

#### Scenario: Missing task title
- **WHEN** a task has no explicit title
- **THEN** the presentation display title falls back to a provider and mode label without exposing the original prompt body.

### Requirement: Presentation summaries expose structured action availability
The system SHALL expose structured action availability for each presented Agent Bridge task so clients can render enabled and disabled lifecycle controls predictably.

#### Scenario: Running task actions
- **WHEN** a task is running
- **THEN** its presentation summary marks wait, inspect logs, inspect transcript when available, inspect status, and stop as actions with `state: "available"` and the corresponding lifecycle tool name.

#### Scenario: Final task actions
- **WHEN** a task is final
- **THEN** its presentation summary marks inspect result, inspect logs, inspect transcript when available, and cleanup when permitted as actions with `state: "available"` and the corresponding lifecycle tool name.

#### Scenario: Unsupported interactive actions
- **WHEN** a provider or task does not support reply or resume
- **THEN** the presentation summary marks reply and resume as actions with `state: "unavailable"`, no lifecycle tool name, and a reason that the provider task is not interactive or resumable.

#### Scenario: Unsafe cleanup action
- **WHEN** a managed worktree task is final but has not been inspected through the final result surface
- **THEN** the presentation summary marks cleanup with `state: "unsafe"` and a reason that managed worktree cleanup is intentional after final result inspection.

### Requirement: Clients can list active and recent agents ergonomically
The system SHALL provide a client-facing way to list active and recent Agent Bridge tasks without requiring clients to process the entire historical task registry by default.

#### Scenario: Default presentation list
- **WHEN** a client requests the default agent presentation list
- **THEN** the response prioritizes non-final tasks first, recent final tasks second by `updatedAt` descending, excludes removed tasks, and includes at most 25 summaries unless the client requests a smaller limit.

#### Scenario: Filtered presentation list
- **WHEN** a client requests agent summaries with filters for status, provider, mode, workspace, title text, or limit
- **THEN** the response includes only matching task summaries up to the requested bound and rejects limits above 100.

#### Scenario: Full history remains intentional
- **WHEN** a client needs the full task registry
- **THEN** the existing raw task lifecycle listing remains available separately from the default native-presentation list.

### Requirement: Presentation preserves verification boundaries
The system SHALL keep native-feeling presentation metadata separate from verification claims about delegated work.

#### Scenario: Provider reports success
- **WHEN** a provider task succeeds and the client renders it as a completed agent
- **THEN** the presentation metadata includes `verificationStatus: "not_verified"` and does not claim project tests, lint, typecheck, build, or requested work verification passed.

#### Scenario: Review packet is available
- **WHEN** a task result includes a review packet
- **THEN** the presentation metadata may link or point to the review packet but does not replace raw logs, diagnostics, diffs, changed files, or caller-run verification.

#### Scenario: Recommended actions remain prose guidance
- **WHEN** a task result includes `reviewPacket.recommendedActions`
- **THEN** structured presentation actions remain separate from the prose recommendations and do not remove or rewrite those recommendations.

### Requirement: Presentation contract is stable across providers
The system SHALL expose the same `presentation` summary shape for Claude, Cursor, Kimi, and Codex tasks even when provider-specific capabilities differ.

#### Scenario: Provider-specific capabilities differ
- **WHEN** two providers support different modes, options, launch profiles, or interactive capabilities
- **THEN** their task summaries use the same presentation fields and represent provider-specific differences through capability metadata and action availability.

#### Scenario: Provider output format differs
- **WHEN** providers emit different stdout, stderr, or structured transcript formats
- **THEN** the presentation summary remains derived from normalized task lifecycle metadata rather than provider-specific raw output parsing.

### Requirement: Presentation exposes progress-aware next actions
The system SHALL include progress-aware metadata and next actions in agent presentation summaries so clients can render running provider agents without treating provider silence as failure.

#### Scenario: Running task recommends observe
- **WHEN** a client reads presentation metadata for a running task
- **THEN** the presentation or top-level `nextActions` includes a primary bounded `agent_observe` action with ready-to-call arguments.

#### Scenario: Presentation actions include observe
- **WHEN** a client reads action availability for a running task
- **THEN** `presentation.actions` includes an `observe` action targeting `agent_observe`.

#### Scenario: Silent task explains output cadence
- **WHEN** a running task has not emitted provider output
- **THEN** presentation metadata includes progress state explaining the expected output cadence and whether silence is still within the recommended provider budget.

#### Scenario: Stop remains explicit
- **WHEN** a task is running but still within the recommended observation budget
- **THEN** `agent_stop` remains available as an explicit lifecycle action but is not ranked ahead of observe, wait, or inspect actions.
