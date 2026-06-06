## MODIFIED Requirements

### Requirement: Tasks expose client-renderable presentation summaries
The system SHALL expose a compact `presentation` summary for Agent Bridge agents through the canonical `agent_*` lifecycle surface so MCP clients can render them as native-feeling agents without parsing raw logs, full agent results, or provider prose.

#### Scenario: Active task summary
- **WHEN** a client requests the presentation summary for a running Agent Bridge agent
- **THEN** the response includes the agent identifier, display title, provider, mode, lifecycle phase, status tone, workspace path, created and updated timestamps, duration when available, and whether a final result is available.

#### Scenario: Final task summary
- **WHEN** a client requests the presentation summary for a final Agent Bridge agent
- **THEN** the response includes final status, completion timestamp, duration, error type when present, changed-file count, transcript availability, whether final or partial provider result evidence was detected, and `verificationStatus: "not_verified"`.

#### Scenario: Stale task summary
- **WHEN** a client requests the presentation summary for an agent marked `failed_stale`
- **THEN** the response uses a final presentation phase, includes the stale error type, and exposes result-inspection actions without implying the agent can be resumed.

#### Scenario: Summary avoids raw payloads
- **WHEN** a client requests a compact presentation summary
- **THEN** the response does not include raw stdout, raw stderr, full git diffs, or full transcript events.

#### Scenario: Missing task title
- **WHEN** an agent has no explicit title
- **THEN** the presentation display title falls back to a provider and mode label without exposing the original prompt body.

### Requirement: Presentation summaries expose structured action availability
The system SHALL expose structured action availability for each presented Agent Bridge agent so clients can render enabled and disabled lifecycle controls predictably.

#### Scenario: Running task actions
- **WHEN** an agent is running
- **THEN** its presentation summary marks wait, inspect logs, inspect transcript when available, inspect status, and stop as actions with `state: "available"` and the corresponding `agent_*` lifecycle tool name.

#### Scenario: Final task actions
- **WHEN** an agent is final
- **THEN** its presentation summary marks inspect result, inspect logs, inspect transcript when available, and cleanup when permitted as actions with `state: "available"` and the corresponding `agent_*` lifecycle tool name.

#### Scenario: Unsupported interactive actions
- **WHEN** a provider or agent does not support reply or resume
- **THEN** the presentation summary marks reply and resume as actions with `state: "unavailable"`, no lifecycle tool name, and a reason that the provider agent is not interactive or resumable.

#### Scenario: Unsafe cleanup action
- **WHEN** a managed worktree agent is final but has not been inspected through the final result surface
- **THEN** the presentation summary marks cleanup with `state: "unsafe"` and a reason that managed worktree cleanup is intentional after final result inspection.

### Requirement: Clients can list active and recent agents ergonomically
The system SHALL provide a client-facing way to list active and recent Agent Bridge agents without requiring clients to process the entire historical registry by default.

#### Scenario: Default presentation list
- **WHEN** a client requests the default agent presentation list
- **THEN** the response prioritizes non-final agents first, recent final agents second by `updatedAt` descending, excludes removed agents, and includes at most 25 summaries unless the client requests a smaller limit.

#### Scenario: Filtered presentation list
- **WHEN** a client requests agent summaries with filters for status, provider, mode, workspace, title text, or limit
- **THEN** the response includes only matching agent summaries up to the requested bound and rejects limits above 100.

#### Scenario: Full history remains intentional
- **WHEN** a client needs the full historical registry
- **THEN** it uses explicit storage inspection or future dedicated tooling rather than a separate advertised raw `task_*` lifecycle list.
