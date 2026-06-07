## MODIFIED Requirements

### Requirement: Codex sandbox denials do not leave active tasks
The system SHALL finalize Codex agents that emit unrecoverable sandbox or approval denial evidence instead of leaving them running indefinitely.

#### Scenario: Codex exits after sandbox denial
- **WHEN** a Codex provider process exits after emitting sandbox or approval denial evidence
- **THEN** `agent_wait`, `agent_status`, and `agent_result` report a final failed agent without requiring `agent_stop`

#### Scenario: Codex hangs after sandbox denial
- **WHEN** a Codex provider process emits sandbox or approval denial evidence and then remains alive
- **THEN** Agent Bridge terminates the provider within a bounded deadline and records a final failed agent with diagnostic evidence

#### Scenario: Codex normal success remains successful
- **WHEN** a Codex provider process completes successfully without sandbox or approval denial evidence
- **THEN** Agent Bridge reports a successful final agent with the intended lifecycle evidence

### Requirement: Codex command investigation is evidence based
The system SHALL verify whether Agent Bridge command construction or prompt rendering contributes to Codex out-of-project patch attempts before changing Codex sandbox mode.

#### Scenario: Codex task preview remains inspectable
- **WHEN** a caller invokes `agent_preview` for Codex implementation work
- **THEN** the preview shows the Codex command, cwd, sandbox mode, redacted prompt transport, and environment keys needed to investigate workspace policy

#### Scenario: Codex adapter changes are regression tested
- **WHEN** the Codex provider adapter command shape, prompt transport, or sandbox mode is changed
- **THEN** stdio tests prove public agent preview, spawn, wait, logs, and result behavior match the updated intended contract
