## ADDED Requirements

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
- **THEN** it summarizes existing status, diagnostics, logs, and exit metadata without parsing provider prose as proof of project correctness
