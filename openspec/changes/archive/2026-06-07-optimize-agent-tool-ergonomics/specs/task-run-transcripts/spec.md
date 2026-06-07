## MODIFIED Requirements

### Requirement: Transcripts are exposed through a bounded inspection surface
The system SHALL expose task transcripts through `agent_observe` events with cursor and
limit controls rather than a separate transcript tool, preserving bounded reads and
unavailability reporting.

#### Scenario: Reading transcript events
- **WHEN** a caller reads `agent_observe` with a `cursor` and `limit`
- **THEN** the response returns normalized transcript and lifecycle events up to the
  requested or configured cap and includes `nextCursor` for subsequent reads.

#### Scenario: Transcript read does not require a separate tool
- **WHEN** a caller needs normalized transcript events (the former `agent_transcript` use
  case)
- **THEN** the caller reads `agent_observe` events with pagination rather than a separate
  transcript tool, and no `agent_transcript` tool is advertised.

#### Scenario: Reading a missing transcript
- **WHEN** a caller observes an agent that predates transcript collection or has no
  transcript artifact
- **THEN** the response returns no transcript events and reports transcript unavailability
  (for example through `progress`/diagnostic metadata) without hiding existing logs or
  result metadata.
