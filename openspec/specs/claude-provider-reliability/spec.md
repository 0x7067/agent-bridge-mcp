# claude-provider-reliability Specification

## Purpose
Define Claude provider readiness, command selection, prompt transport, bounded diagnostics, output parsing, and troubleshooting behavior for reliable delegated Claude tasks.
## Requirements
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
- **WHEN** Claude provider diagnostics are returned through `providers_check`, `agent_result`, or task logs
- **THEN** prompts, API tokens, OAuth tokens, and non-allowlisted environment values are absent from diagnostic fields.

#### Scenario: Claude host runner is unavailable
- **WHEN** the Claude provider is configured to use a host runner but the host runner cannot be reached
- **THEN** the task or smoke result reports a host-runner failure category with an actionable setup message.

### Requirement: Claude provider command selection is explicit and explainable
The system SHALL make Claude provider command selection deterministic and explainable for `claude-p` and native `claude -p`.

#### Scenario: Claude-p is explicitly configured
- **WHEN** `CLAUDE_P_BIN` or an equivalent adapter test override is configured
- **THEN** the Claude provider uses that `claude-p` path and reports that selection in preview and provider check diagnostics.

#### Scenario: Native Claude is explicitly configured
- **WHEN** native `CLAUDE_BIN` or an equivalent adapter test override is configured and no explicit `claude-p` path overrides it
- **THEN** the Claude provider uses native `claude -p` and reports that selection in preview and provider check diagnostics.

#### Scenario: Claude-p is unhealthy while native Claude is available
- **WHEN** `claude-p` fails a smoke probe and native `claude -p` is discoverable or configured
- **THEN** the provider check response recommends the native Claude path as the next troubleshooting or fallback step without silently changing the task provider command.

### Requirement: Claude prompt transport is robust for task prompts
The system SHALL pass Claude task prompts in a way that preserves task content, avoids accidental flag parsing or shell interpretation, and does not place task prompt text in process argument lists.

#### Scenario: Multiline prompts are passed intact
- **WHEN** a Claude task prompt contains multiple lines, quotes, shell metacharacters, or leading dashes
- **THEN** the provider command passes the rendered task prompt as data and the spawned provider receives the original prompt content without shell expansion.

#### Scenario: Prompt is not exposed through argv
- **WHEN** the bridge spawns a Claude provider task
- **THEN** the rendered task prompt is transported through stdin, an input file, or another provider-supported non-argv mechanism.

#### Scenario: Large prompts use a supported transport
- **WHEN** a Claude task prompt is too large for reliable positional argument transport
- **THEN** the provider command uses a `claude-p` or native Claude supported stdin/input-file transport, or rejects the task before spawning with an actionable validation error.

#### Scenario: Selected Claude path lacks non-argv prompt transport
- **WHEN** the selected Claude provider command does not support stdin, input-file, or another safe non-argv prompt transport
- **THEN** the bridge rejects the task before spawning and returns an actionable validation error.

### Requirement: Claude provider output cannot corrupt MCP stdio
The system SHALL isolate provider stdout/stderr from MCP protocol stdout, including noisy terminal probe output emitted by `claude-p` or Claude Code.

#### Scenario: Provider emits terminal probe output
- **WHEN** the Claude provider emits terminal probe sequences, progress text, or other non-JSON output
- **THEN** those bytes are captured in task logs or diagnostics and are never written to MCP server stdout.

#### Scenario: Provider emits valid JSON with surrounding noise
- **WHEN** the Claude provider emits a valid result with surrounding non-result output
- **THEN** the bridge extracts or classifies the result deterministically and reports any ignored noise through bounded diagnostics.

### Requirement: Claude troubleshooting is documented
The system SHALL document the Claude provider reliability model and troubleshooting workflow.

#### Scenario: User investigates unreliable Claude provider
- **WHEN** a user reads the README provider troubleshooting section
- **THEN** the docs explain `providers_check` with and without `smoke: true`, `CLAUDE_P_BIN`, `CLAUDE_BIN`, known `claude-p` fragility, timeout symptoms, host-runner setup for macOS Keychain-backed auth, and how to switch to native `claude -p` only as an optional troubleshooting path.

#### Scenario: User needs upstream claude-p context
- **WHEN** a user follows the Claude provider troubleshooting documentation
- **THEN** the docs link to upstream `claude-p` compatibility documentation and call out that changes in Claude Code terminal behavior can require bridge updates.
