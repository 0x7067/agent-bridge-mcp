## ADDED Requirements

### Requirement: Claude provider uses an owned interactive runner
The system SHALL execute Claude provider tasks through Agent Bridge-owned interactive Claude runner code instead of upstream `claude-p` or native `claude -p` print mode.

#### Scenario: Spawning a Claude task
- **WHEN** a caller spawns a task with provider `claude`
- **THEN** the bridge launches the official interactive `claude` CLI through the owned runner path.
- **AND** the bridge does not execute upstream `claude-p` or native `claude -p` as a fallback.

#### Scenario: Previewing Claude launch strategy
- **WHEN** a caller previews a Claude task
- **THEN** the preview identifies the owned interactive Claude launch strategy.
- **AND** the preview does not recommend native print mode or upstream `claude-p` fallback.

### Requirement: Claude runner drives a real PTY session
The system SHALL run interactive Claude in a real pseudo-terminal and handle the terminal startup behavior required for Claude Code to begin accepting input.

#### Scenario: Claude emits terminal startup probes
- **WHEN** interactive Claude writes terminal capability probes during startup
- **THEN** the owned runner responds with supported terminal answers needed for the session to continue.
- **AND** the probe bytes remain isolated from MCP protocol stdout.

#### Scenario: Claude reaches input-ready state
- **WHEN** interactive Claude is ready for user input
- **THEN** the owned runner injects the rendered task prompt as terminal input without exposing the prompt in process argv.

### Requirement: Claude runner captures completion through transcript data
The system SHALL detect Claude task completion through runner-owned Stop-hook/transcript handling and convert the final assistant output into the existing task result surfaces.

#### Scenario: Stop hook reports transcript path
- **WHEN** Claude finishes and the Stop hook payload contains a transcript path
- **THEN** the runner reads the transcript, extracts the final assistant result, and returns provider output compatible with existing task result parsing.

#### Scenario: Transcript path is unsafe
- **WHEN** a Stop hook payload reports a transcript path that is not an absolute regular readable file path accepted by the runner's transcript policy
- **THEN** the runner rejects that transcript path, records a bounded diagnostic, and falls back to Stop-hook `last_assistant_message` only when available.

#### Scenario: Transcript is missing or malformed
- **WHEN** Claude exits without a usable Stop-hook payload or transcript result
- **THEN** the task fails with `provider_output_error` and bounded diagnostics.

#### Scenario: Stop hook fires before transcript flush completes
- **WHEN** the Stop hook reports a transcript path before the final assistant message is visible in the transcript file
- **THEN** the runner retries transcript parsing within a bounded budget.
- **AND** it falls back to the Stop-hook `last_assistant_message` field when available after the bounded retry budget is exhausted.

#### Scenario: Transcript parser priority
- **WHEN** both transcript data and Stop-hook `last_assistant_message` are available
- **THEN** the runner prefers the final assistant message parsed from transcript JSONL records.
- **AND** it uses `last_assistant_message` only as a bounded fallback when transcript parsing cannot produce a final assistant message.

#### Scenario: Transcript path policy
- **WHEN** the runner validates a transcript path from a hook payload
- **THEN** the path must be absolute, canonicalizable, a regular readable file after canonicalization, and not a symlink escape.
- **AND** the runner rejects the path with bounded diagnostics if those checks fail.

### Requirement: Claude runner classifies StopFailure separately
The system SHALL handle Claude `StopFailure` hook payloads as provider/API failures rather than malformed transcript output.

#### Scenario: StopFailure reports authentication failure
- **WHEN** a Claude StopFailure hook payload reports an authentication, billing, rate-limit, model, or server error
- **THEN** the runner maps it to an actionable provider failure category and bounded diagnostic.
- **AND** it does not report the task as successful.

#### Scenario: StopFailure contains partial assistant text
- **WHEN** a StopFailure payload includes `last_assistant_message`
- **THEN** the runner may include the redacted partial message in diagnostics.
- **AND** it does not treat the partial message as a successful final result.

#### Scenario: StopFailure category mapping
- **WHEN** a StopFailure payload reports a known error class
- **THEN** the runner maps it to one of `claude_auth_error`, `claude_billing_error`, `claude_rate_limit`, `claude_model_unavailable`, or `claude_api_error`.
- **AND** unknown StopFailure classes map to `claude_api_error` with bounded diagnostics.

#### Scenario: StopFailure input mapping
- **WHEN** a StopFailure payload reports `auth_error`, `authentication_error`, or equivalent Claude auth failure
- **THEN** the runner maps it to `claude_auth_error`.
- **WHEN** it reports `billing_error`, `credit_exhausted`, or equivalent billing failure
- **THEN** the runner maps it to `claude_billing_error`.
- **WHEN** it reports `rate_limit` or equivalent throttling failure
- **THEN** the runner maps it to `claude_rate_limit`.
- **WHEN** it reports `model_unavailable` or equivalent model selection failure
- **THEN** the runner maps it to `claude_model_unavailable`.

### Requirement: Claude runner owns temporary settings safely
The system SHALL use runner-owned temporary Claude settings for automation hooks without overwriting user configuration.

#### Scenario: Runner starts a Claude session
- **WHEN** the owned runner starts Claude
- **THEN** it supplies temporary settings needed for prompt injection and Stop-hook capture.
- **AND** it does not permanently edit `~/.claude` or project configuration files.

#### Scenario: Runner creates temporary settings
- **WHEN** the owned runner writes temporary settings for a Claude session
- **THEN** it creates them under a runner-owned temporary directory with owner-only permissions.
- **AND** settings files are written with owner-only file permissions before Claude can read them.

#### Scenario: SessionStart hook runs
- **WHEN** a runner-owned `SessionStart` hook receives a lifecycle payload
- **THEN** it may relay startup metadata to the runner-owned FIFO.
- **AND** it must not inject user-visible text into Claude context unless the runner explicitly uses it as part of prompt injection.

#### Scenario: Runner hook command runs
- **WHEN** a runner-owned hook command receives a lifecycle payload
- **THEN** it writes only to the runner-owned FIFO relay channel.
- **AND** it avoids stdout that would be injected into Claude context unless explicitly intended by the runner.

#### Scenario: Runner exits
- **WHEN** the owned runner finishes, times out, or is interrupted
- **THEN** it cleans up temporary files it created where cleanup is possible.

### Requirement: Claude runner detects blocking setup prompts
The system SHALL fail fast with actionable diagnostics when interactive Claude cannot proceed because the local CLI is not initialized or is waiting for user setup.

#### Scenario: Claude requires login or first-run setup
- **WHEN** the PTY output indicates a login, first-run, terms, authentication, or setup prompt instead of task processing
- **THEN** the runner fails the task with an actionable Claude setup diagnostic rather than waiting until the normal task timeout.

#### Scenario: Setup prompt detection is bounded
- **WHEN** Claude does not reach input-ready state before the startup deadline or emits known setup prompt signatures
- **THEN** the runner classifies the failure as `claude_setup_required`.

### Requirement: Claude runner preserves Agent Bridge task safety
The system SHALL preserve Agent Bridge mode restrictions, cwd validation, timeout handling, redaction, and output isolation for Claude tasks.

#### Scenario: Research or review task
- **WHEN** a Claude task is launched in `research` or `review` mode
- **THEN** the runner configures Claude permissions so file writes and shell execution are not allowed by default.

#### Scenario: Command task
- **WHEN** a Claude task is launched in `command` mode
- **THEN** the runner configures Claude permissions to allow bounded shell execution and read/search tools.
- **AND** it disallows file edits and writes by default.

#### Scenario: Implement task
- **WHEN** a Claude task is launched in `implement` mode
- **THEN** the runner configures Claude permissions to allow the edit and command capabilities required for implementation under Agent Bridge cwd and timeout controls.

#### Scenario: Task times out
- **WHEN** an owned Claude runner task exceeds its configured timeout
- **THEN** the bridge terminates and reaps the Claude child process tree where supported.
- **AND** the task result includes a timeout failure category with bounded stdout/stderr or transcript diagnostics.

### Requirement: Claude runner supports deterministic test backends
The system SHALL support deterministic fake owned-runner backends for automated tests without making direct execution a production launch path.

#### Scenario: Test backend is configured
- **WHEN** tests configure a fake owned-runner backend
- **THEN** Claude readiness and task lifecycle tests can exercise PTY, hook, transcript, timeout, and failure behavior without live Claude auth, Keychain, network, or model usage.

#### Scenario: Production backend is selected
- **WHEN** the MCP server runs in normal production configuration
- **THEN** Claude launch readiness still requires the owned host-runner path and does not rely on the fake or direct test backend.

### Requirement: Claude runner parity is scoped to Agent Bridge
The system SHALL provide final-result behavior sufficient for Agent Bridge task lifecycle tools without promising byte-for-byte `claude -p` compatibility.

#### Scenario: Caller reads task result
- **WHEN** a Claude task completes successfully
- **THEN** `agent_result` returns the existing task result structure with Claude's final report, changed-file metadata when available, evidence, risks, and next steps.

#### Scenario: Caller requests unsupported print-mode parity
- **WHEN** a behavior only exists in native print mode, such as exact per-token stream-json event parity
- **THEN** the Claude provider does not expose that behavior unless the owned runner implements it explicitly.
