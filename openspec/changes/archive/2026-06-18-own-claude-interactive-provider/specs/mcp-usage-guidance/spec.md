## ADDED Requirements

### Requirement: Guidance describes owned Claude runner workflow
The system SHALL update server-discoverable guidance so Claude setup and troubleshooting describe the owned interactive host-runner workflow rather than print-mode fallback.

#### Scenario: Host-runner lifecycle guidance
- **WHEN** a client reads Claude host-runner lifecycle guidance
- **THEN** the guidance explains owned interactive Claude runner startup, protocol mismatch handling, workspace-policy alignment, smoke checks, and restart guidance.
- **AND** it does not recommend native `claude -p` or upstream `claude-p` fallback.

#### Scenario: Provider capability guidance
- **WHEN** a client reads provider capability guidance
- **THEN** Claude is described as using the owned interactive PTY/hook/transcript runner.
