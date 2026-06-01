## ADDED Requirements

### Requirement: Provider runtime metadata supports skill validation
The system SHALL expose or make available deterministic provider runtime metadata sufficient to validate repo-owned provider skill guidance.

#### Scenario: Skill validation reads supported providers
- **WHEN** provider skill validation runs
- **THEN** it can determine the same provider names exposed through `providers_list`

#### Scenario: Skill validation reads supported modes
- **WHEN** provider skill validation runs for a supported provider
- **THEN** it can determine the same supported modes exposed for that provider through `providers_list`

#### Scenario: Skill validation does not duplicate command construction
- **WHEN** provider skill validation reads provider runtime metadata
- **THEN** it validates provider names and modes without duplicating provider command construction, environment allowlists, readiness probes, or launch strategy logic

### Requirement: Provider adapters remain runtime authority
The system SHALL keep provider adapters as the authority for Agent Bridge task execution even when provider skill guidance exists.

#### Scenario: Task preview ignores skill prose
- **WHEN** a caller invokes `task_preview`
- **THEN** the previewed command descriptor is built from provider adapter runtime logic and not from provider skill markdown

#### Scenario: Task spawn ignores skill prose
- **WHEN** a caller invokes `task_spawn`
- **THEN** the spawned provider command is built from provider adapter runtime logic and not from provider skill markdown

#### Scenario: Runtime behavior is independent of skill edits
- **WHEN** a provider skill changes direct CLI runbook prose without changing provider adapter code
- **THEN** Agent Bridge task execution behavior remains derived from provider adapter runtime logic rather than skill prose
