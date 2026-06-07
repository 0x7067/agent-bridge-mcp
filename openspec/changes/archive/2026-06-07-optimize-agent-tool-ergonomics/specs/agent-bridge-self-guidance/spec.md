## MODIFIED Requirements

### Requirement: Task surfaces expose ranked next actions
The system SHALL derive ranked next actions from each inspectable agent record and emit
them exactly once per response as a single `next` list, ranked with the primary action
first.

#### Scenario: Single emission per response
- **WHEN** any agent read tool (`agent_observe`, `agent_result`, `agent_list`) returns next
  actions
- **THEN** the ranked actions appear once as a top-level `next` array and are not duplicated
  inside nested `agent`, `presentation`, or `progress` objects.

#### Scenario: Running agent next action
- **WHEN** an agent is queued or running
- **THEN** `next` ranks a bounded `agent_observe` (or finality wait via `until: "final"`)
  first, with ready-to-call arguments.

#### Scenario: Final uninspected agent next action
- **WHEN** an agent is final and its result has not been inspected
- **THEN** `next` ranks `agent_result` first, before any cleanup action.

#### Scenario: Managed worktree cleanup remains gated
- **WHEN** a managed-worktree agent is final but the final result has not been inspected
- **THEN** cleanup is not the primary `next` action and is marked unsafe with a reason.

#### Scenario: Failed agent next action
- **WHEN** an agent is failed, stopped, or stale
- **THEN** `next` recommends `agent_result` evidence inspection (logs/diagnostics via result
  sections) before any rerun.

### Requirement: Next actions are machine-actionable and safety-aware
The system SHALL make `next` metadata usable by clients without hiding safety state, using
the consolidated eight-tool surface as call targets.

#### Scenario: Next action includes call target
- **WHEN** a `next` item is returned
- **THEN** it includes an action id, target tool name when applicable, ready-to-call
  arguments, and a safety classification, with a short reason
- **AND** target tool names are drawn from the consolidated surface (for example
  `agent_observe`, `agent_result`, `agent_remove`) and never name a removed tool.

#### Scenario: Verification remains caller-owned
- **WHEN** a `next` action follows provider success
- **THEN** it does not claim the original user request is verified and it directs the caller
  toward project verification when appropriate.
