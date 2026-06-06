## ADDED Requirements

### Requirement: Guidance teaches a compact default workflow
The system SHALL teach a compact default Agent Bridge workflow in prompts and guidance resources.

#### Scenario: Caller workflow guidance uses compact path
- **WHEN** a client reads caller workflow guidance through prompts or resources
- **THEN** the guidance presents the primary path as setup check when uncertain, optional focused readiness check, spawn, observe, result inspection, caller-owned verification, and intentional cleanup.

#### Scenario: Diagnostic tools are contextual
- **WHEN** guidance names `providers_check`, `agent_preview`, `agent_list`, `agent_status`, `agent_wait`, `agent_logs`, or `agent_transcript`
- **THEN** it describes why a caller would use that tool for focused readiness, launch inspection, native presentation, simple finality, raw evidence, transcript evidence, or recovery.
- **AND** it does not present every diagnostic tool as required for normal successful delegation.

#### Scenario: Stalled recovery remains explicit
- **WHEN** guidance describes stalled, failed, denied, or unclear provider behavior
- **THEN** it still instructs callers to use bounded diagnostic tools such as `agent_observe`, `agent_logs`, `agent_transcript`, `agent_status`, `agent_stop`, and final `agent_result` as appropriate.

### Requirement: Guidance preserves manual fallback without expanding the default path
The system SHALL preserve enough manual lifecycle guidance for clients that do not consume structured content or `nextActions`.

#### Scenario: Manual fallback names escape hatches
- **WHEN** a client does not use initialization instructions, structured content, or `nextActions`
- **THEN** prompts and resources still document how to use status, wait, logs, transcript, stop, and cleanup tools for manual inspection.
- **AND** the documented default path remains smaller than the full lifecycle tool set.
