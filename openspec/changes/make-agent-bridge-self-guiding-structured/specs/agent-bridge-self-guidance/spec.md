## ADDED Requirements

### Requirement: Server initialization provides concise workflow instructions
The system SHALL return MCP initialization instructions that summarize the safe Agent Bridge workflow and verification boundary.

#### Scenario: Initialize includes instructions
- **WHEN** a caller sends `initialize`
- **THEN** the response includes an `instructions` string describing the recommended `doctor`, readiness, spawn, wait/log, result inspection, verification, and cleanup workflow.

#### Scenario: Instructions are front-loaded
- **WHEN** a caller reads the first 512 characters of the initialization instructions
- **THEN** those characters include the task lifecycle boundary that provider output is evidence and caller-owned verification is still required.

### Requirement: JSON tool results include structured content
The system SHALL include structured JSON content for Agent Bridge tools that return JSON payloads while preserving the existing text content.

#### Scenario: Tool result includes structured content
- **WHEN** a JSON-returning Agent Bridge tool succeeds
- **THEN** the MCP tool result includes the existing serialized JSON text content.
- **AND** the result includes `structuredContent` containing the same semantic JSON payload.

#### Scenario: Text compatibility is preserved
- **WHEN** an existing caller parses the first text content item as JSON
- **THEN** the caller can continue to parse the result without depending on `structuredContent`.

### Requirement: Stable tools expose output schemas
The system SHALL expose MCP output schemas for stable Agent Bridge tool result shapes where the schema can be maintained without constraining provider-specific diagnostics.

#### Scenario: Tool schema lists stable output schema
- **WHEN** a caller sends `tools/list`
- **THEN** stable Agent Bridge JSON tools include `outputSchema` metadata for top-level fields that clients can validate.

#### Scenario: Provider diagnostics remain flexible
- **WHEN** a tool result includes provider-specific diagnostics or excerpts
- **THEN** the output schema does not require volatile provider-specific nested fields beyond documented generic containers.

### Requirement: Task surfaces expose ranked next actions
The system SHALL derive ranked next actions from each inspectable task record.

#### Scenario: Running task next action
- **WHEN** a task is queued or running
- **THEN** its presentation metadata includes a primary `nextActions` item recommending a bounded `task_wait` or incremental inspection action with ready-to-call arguments.

#### Scenario: Final uninspected task next action
- **WHEN** a task is final and its result has not been inspected
- **THEN** its presentation metadata includes a primary `nextActions` item recommending `task_result` before cleanup.

#### Scenario: Managed worktree cleanup remains gated
- **WHEN** a managed-worktree task is final but the final result has not been inspected
- **THEN** cleanup is not the primary next action and remains marked unsafe with a reason.

#### Scenario: Failed task next action
- **WHEN** a task is failed, stopped, or stale
- **THEN** its `nextActions` metadata recommends result/log/diagnostic inspection before any rerun.

### Requirement: Next actions are machine-actionable and safety-aware
The system SHALL make next-action metadata usable by clients without hiding safety state.

#### Scenario: Next action includes call target
- **WHEN** `nextActions` metadata is returned
- **THEN** it includes an action id, target tool name when applicable, arguments, state, reason, and safety classification.

#### Scenario: Verification remains caller-owned
- **WHEN** a next action follows provider success
- **THEN** it does not claim the original user request is verified and it directs the caller toward project verification when appropriate.
