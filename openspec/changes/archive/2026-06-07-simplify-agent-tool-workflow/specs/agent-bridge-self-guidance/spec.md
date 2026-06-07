## ADDED Requirements

### Requirement: Self-guidance distinguishes primary and diagnostic lifecycle tools
The system SHALL present a small primary Agent Bridge workflow while preserving diagnostic lifecycle tools as fallback surfaces.

#### Scenario: Initialize names primary workflow first
- **WHEN** a caller sends `initialize`
- **THEN** the instructions describe `agent_spawn`, `agent_observe`, `agent_result`, caller-owned verification, and optional cleanup as the primary lifecycle path.
- **AND** the instructions identify readiness, launch-preview, status, wait, log, and transcript tools as focused or diagnostic surfaces rather than required default steps.

#### Scenario: Running task next action prefers observe
- **WHEN** a task is queued or running
- **THEN** its first machine-actionable `nextActions` item targets `agent_observe` with ready-to-call arguments.
- **AND** lower-level status, wait, log, stop, or transcript actions remain available only as subsequent inspection or control options when applicable.

#### Scenario: Final task next action prefers result inspection
- **WHEN** a task is final and its result has not been inspected
- **THEN** its first machine-actionable `nextActions` item targets `agent_result`.
- **AND** cleanup remains unavailable or unsafe until final evidence has been inspected when managed worktree safety requires it.
