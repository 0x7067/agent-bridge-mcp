## ADDED Requirements

### Requirement: Guidance points operators to doctor
The system SHALL recommend `doctor` as the first troubleshooting step for Agent Bridge setup and readiness issues.

#### Scenario: Caller workflow guidance mentions doctor
- **WHEN** a client reads caller workflow guidance
- **THEN** the guidance tells operators to run `doctor` before deeper provider readiness or host-runner troubleshooting.

#### Scenario: Host-runner guidance mentions doctor
- **WHEN** a client reads Claude host-runner lifecycle guidance
- **THEN** the guidance tells operators to use `doctor` to inspect socket reachability and workspace-policy mismatch.

#### Scenario: Result guidance remains separate
- **WHEN** a client reads task result inspection guidance
- **THEN** the guidance keeps `doctor` separate from task-result verification and does not imply doctor verifies delegated work.
