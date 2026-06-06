## MODIFIED Requirements

### Requirement: Public tool arguments remain strict
The system SHALL continue to reject unknown fields inside each tool's public `arguments` object.

#### Scenario: Unknown argument remains rejected
- **WHEN** a caller invokes `agent_preview` with an unsupported argument such as `maxTurns` inside `arguments`
- **THEN** the tool response is an error that identifies the unknown argument

#### Scenario: Envelope metadata does not disable argument validation
- **WHEN** a caller invokes `agent_preview` with `_meta` on the envelope and an unsupported field inside `arguments`
- **THEN** the tool response is still an unknown-argument error
