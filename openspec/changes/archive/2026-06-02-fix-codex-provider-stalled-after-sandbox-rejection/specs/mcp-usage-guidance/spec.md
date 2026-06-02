## ADDED Requirements

### Requirement: Guidance explains Codex sandbox denial recovery
The system SHALL document how callers should investigate and recover from Codex sandbox, approval, or out-of-workspace patch denials.

#### Scenario: Guidance names Codex denial symptoms
- **WHEN** a client reads Agent Bridge recovery, safety, or provider guidance
- **THEN** the guidance mentions Codex patch rejection, sandbox denial, approval denial, or out-of-workspace write symptoms as setup or prompt-scope issues to inspect

#### Scenario: Guidance recommends bounded lifecycle inspection
- **WHEN** guidance describes recovering from Codex denial failures
- **THEN** it tells callers to use bounded `task_wait`, `task_logs`, `task_status`, and final `task_result` inspection instead of waiting indefinitely

#### Scenario: Guidance preserves safety boundary
- **WHEN** guidance describes follow-up actions for Codex denial failures
- **THEN** it tells callers to inspect `cwd`, workspace policy, prompt scope, and isolation strategy before retrying
- **AND** it does not tell callers to silently relax sandbox permissions
