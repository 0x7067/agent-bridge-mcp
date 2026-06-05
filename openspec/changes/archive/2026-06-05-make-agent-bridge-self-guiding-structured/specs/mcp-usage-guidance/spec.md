## ADDED Requirements

### Requirement: Guidance mirrors initialization instructions
The system SHALL keep MCP prompts and resources aligned with the initialization instructions for the standard Agent Bridge workflow.

#### Scenario: Caller workflow guidance names self-guided surfaces
- **WHEN** a client reads caller workflow guidance
- **THEN** the guidance mentions initialization instructions, structured tool results, next-action metadata, and the existing lifecycle tools.

#### Scenario: Guidance preserves fallback path
- **WHEN** a client does not use initialization instructions or structured content
- **THEN** prompts and resources still describe the manual lifecycle using `doctor`, `providers_check`, `task_spawn`, `task_wait`, `task_logs`, `task_transcript`, `task_result`, and `task_remove`.

### Requirement: Guidance explains doctor launch readiness
The system SHALL explain that setup health and provider launch readiness are separate concerns.

#### Scenario: Readiness guidance
- **WHEN** a client reads setup or caller workflow guidance
- **THEN** the guidance explains that version-only provider checks can leave providers available but not startup-verified or launchable.

#### Scenario: Smoke remains opt-in
- **WHEN** guidance recommends startup verification
- **THEN** it tells callers to use bounded smoke checks intentionally rather than making live provider smoke part of default verification.
