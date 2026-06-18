# claude-provider-reliability Specification

## Purpose
Define Claude provider readiness, command selection, prompt transport, bounded diagnostics, output parsing, and troubleshooting behavior for reliable delegated Claude tasks.
## Requirements
### Requirement: Claude provider health distinguishes binary presence from startup readiness
The system SHALL distinguish official interactive Claude binary availability from owned-runner startup readiness and task execution readiness, including whether the selected Claude task path runs through the configured owned host runner.

#### Scenario: Version check reports binary presence only
- **WHEN** a caller invokes `providers_check` without smoke probes for the Claude provider
- **THEN** the response reports whether the official interactive `claude` binary can answer its version command.
- **AND** it sets `startupVerified` to `false` and does not mark Claude launchable.

#### Scenario: Smoke check exercises the owned Claude task path
- **WHEN** a caller invokes `providers_check` with `smoke: true` for the Claude provider
- **THEN** the smoke probe uses the same owned interactive runner, host-runner strategy, environment policy, PTY prompt transport, hook handling, transcript parsing, and output expectations used for a real smoke task.

#### Scenario: Version success with owned-runner smoke failure is not reported as healthy
- **WHEN** the official interactive Claude binary answers a version probe but the owned runner fails startup, prompt injection, hook completion, or transcript parsing during a smoke probe
- **THEN** the response reports the provider as unavailable or degraded for task execution and includes an actionable failure category.

#### Scenario: Host-runner smoke reports launch strategy
- **WHEN** the Claude provider is configured to use the owned host runner and a caller invokes `providers_check` with `smoke: true`
- **THEN** the response reports that the Claude smoke path used owned host-runner execution.

### Requirement: Claude provider diagnostics are bounded and actionable
The system SHALL surface bounded Claude provider diagnostics that identify likely owned-runner failure classes without exposing prompts, secrets, terminal transcripts, or unrelated host environment values.

#### Scenario: Owned runner startup hangs
- **WHEN** the owned Claude runner does not reach input-ready state or task completion before the configured timeout
- **THEN** the task or smoke result reports a timeout failure category, the configured timeout, the owned-runner command label, and capped PTY/log excerpts.

#### Scenario: Owned runner exits without a usable result
- **WHEN** the owned Claude runner exits zero or non-zero without a parseable Stop payload, StopFailure payload, transcript result, or fallback assistant message
- **THEN** the task or smoke result reports a provider output failure category and includes capped excerpts sufficient for troubleshooting.

#### Scenario: Claude diagnostics redact sensitive data
- **WHEN** Claude provider diagnostics are returned through `providers_check`, `agent_result`, task logs, or doctor output
- **THEN** prompts, API tokens, OAuth tokens, hook payload secrets, and non-allowlisted environment values are absent from diagnostic fields.

#### Scenario: Claude host runner is unavailable
- **WHEN** the Claude provider is configured to use the owned host runner but the host runner cannot be reached
- **THEN** the task or smoke result reports a host-runner failure category with an actionable setup message.

### Requirement: Claude provider command selection is explicit and explainable
The system SHALL make Claude provider command selection deterministic and explainable for the owned interactive runner only.

#### Scenario: Owned Claude runner is selected
- **WHEN** a Claude task, preview, or smoke probe is built
- **THEN** the Claude provider reports command selection as the owned interactive runner.

#### Scenario: Native Claude print mode is available
- **WHEN** native `claude -p` is discoverable
- **THEN** the Claude provider does not use native print mode as a fallback.
- **AND** provider checks do not recommend native print mode as the next step.

#### Scenario: CLAUDE_BIN is configured
- **WHEN** `CLAUDE_BIN` is configured
- **THEN** the Claude provider treats it only as the official interactive `claude` executable override.
- **AND** it does not append `-p` or use it as native print mode.

### Requirement: Claude prompt transport is robust for task prompts
The system SHALL pass Claude task prompts through owned interactive PTY input and hook/session data in a way that preserves task content, avoids shell interpretation, and does not place task prompt text in process argument lists.

#### Scenario: Multiline prompts are passed intact
- **WHEN** a Claude task prompt contains multiple lines, quotes, shell metacharacters, or leading dashes
- **THEN** the owned runner passes the rendered task prompt as terminal/session data and Claude receives the original prompt content without shell expansion.

#### Scenario: Prompt is not exposed through argv
- **WHEN** the bridge spawns a Claude provider task
- **THEN** the rendered task prompt is not present in process argv for the MCP server, host runner, hook helper, shell wrapper, or Claude child.

#### Scenario: Large prompts use bounded transport
- **WHEN** a Claude task prompt approaches the task prompt size limit
- **THEN** the owned runner either transports it through bounded PTY/session data within the existing task prompt size limit or rejects the task before spawning with an actionable validation error.

### Requirement: Claude provider output cannot corrupt MCP stdio
The system SHALL isolate owned-runner PTY output, hook relay data, stdout, and stderr from MCP protocol stdout.

#### Scenario: Provider emits terminal probe output
- **WHEN** the owned runner or Claude emits terminal probe sequences, progress text, or other non-result output
- **THEN** those bytes are captured in task logs or diagnostics and are never written to MCP server stdout.

#### Scenario: Provider emits valid result with surrounding noise
- **WHEN** the owned runner captures a valid Stop/transcript result with surrounding terminal noise
- **THEN** the bridge extracts or classifies the result deterministically and reports ignored noise only through bounded diagnostics.

### Requirement: Claude troubleshooting is documented
The system SHALL document the owned Claude provider reliability model and troubleshooting workflow.

#### Scenario: User investigates unreliable Claude provider
- **WHEN** a user reads the README provider troubleshooting section
- **THEN** the docs explain owned interactive runner startup, PTY/probe handling, Stop-hook/transcript capture, host-runner setup, bounded diagnostics, and smoke checks.
- **AND** the docs do not recommend switching to native `claude -p`.

#### Scenario: User needs upstream claude-p context
- **WHEN** a user follows the Claude provider troubleshooting documentation
- **THEN** the docs may link to upstream `claude-p` as historical/reference context.
- **AND** the docs state that Agent Bridge owns its Claude runner instead of depending on upstream `claude-p` at runtime.
