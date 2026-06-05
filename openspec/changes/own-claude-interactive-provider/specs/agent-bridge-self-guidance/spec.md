## ADDED Requirements

### Requirement: Initialization guidance reflects owned Claude readiness
The system SHALL keep initialization instructions aligned with owned Claude host-runner readiness.

#### Scenario: Initialize mentions Claude readiness
- **WHEN** a caller reads initialization instructions
- **THEN** Claude readiness guidance distinguishes official interactive Claude binary presence from owned host-runner startup verification.
- **AND** it does not imply direct `claude-p` or native `claude -p` fallback.
