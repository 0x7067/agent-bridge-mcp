## MODIFIED Requirements

### Requirement: Claude provider health distinguishes binary presence from startup readiness
The system SHALL distinguish Claude provider binary availability from startup readiness and task execution readiness, including whether the selected Claude task path runs through a configured host runner.

#### Scenario: Version check reports binary presence only
- **WHEN** a caller invokes `providers_check` without smoke probes for the Claude provider
- **THEN** the response reports whether the selected Claude binary can answer its version command and sets `startupVerified` to `false`.

#### Scenario: Smoke check exercises the selected Claude task path
- **WHEN** a caller invokes `providers_check` with `smoke: true` for the Claude provider
- **THEN** the smoke probe uses the same Claude command selection, shell initialization, environment policy, launch strategy, and output expectations used for a real smoke task.

#### Scenario: Version success with smoke failure is not reported as healthy
- **WHEN** the selected Claude binary answers a version probe but fails startup or task execution during a smoke probe
- **THEN** the response reports the provider as unavailable or degraded for task execution and includes an actionable failure category.

#### Scenario: Host-runner smoke reports launch strategy
- **WHEN** the Claude provider is configured to use a host runner and a caller invokes `providers_check` with `smoke: true`
- **THEN** the response reports that the Claude smoke path used host-runner execution.

### Requirement: Claude provider diagnostics are bounded and actionable
The system SHALL surface bounded Claude provider diagnostics that identify likely failure classes without exposing prompts, secrets, or unrelated host environment values.

#### Scenario: Claude-p startup hangs
- **WHEN** the selected `claude-p` command does not produce a result before the configured timeout
- **THEN** the task or smoke result reports a timeout failure category, the configured timeout, the selected provider path label, and capped stdout/stderr excerpts.

#### Scenario: Claude-p exits without a usable result
- **WHEN** the selected `claude-p` command exits zero or non-zero without a parseable provider result
- **THEN** the task or smoke result reports a provider output failure category and includes capped stdout/stderr excerpts sufficient for troubleshooting.

#### Scenario: Claude diagnostics redact sensitive data
- **WHEN** Claude provider diagnostics are returned through `providers_check`, `task_result`, or task logs
- **THEN** prompts, API tokens, OAuth tokens, and non-allowlisted environment values are absent from diagnostic fields.

#### Scenario: Claude host runner is unavailable
- **WHEN** the Claude provider is configured to use a host runner but the host runner cannot be reached
- **THEN** the task or smoke result reports a host-runner failure category with an actionable setup message.

### Requirement: Claude troubleshooting is documented
The system SHALL document the Claude provider reliability model and troubleshooting workflow.

#### Scenario: User investigates unreliable Claude provider
- **WHEN** a user reads the README provider troubleshooting section
- **THEN** the docs explain `providers_check` with and without `smoke: true`, `CLAUDE_P_BIN`, `CLAUDE_BIN`, known `claude-p` fragility, timeout symptoms, host-runner setup for macOS Keychain-backed auth, and how to switch to native `claude -p` only as an optional troubleshooting path.

#### Scenario: User needs upstream claude-p context
- **WHEN** a user follows the Claude provider troubleshooting documentation
- **THEN** the docs link to upstream `claude-p` compatibility documentation and call out that changes in Claude Code terminal behavior can require bridge updates.
