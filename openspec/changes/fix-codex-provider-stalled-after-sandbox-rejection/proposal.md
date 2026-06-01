## Why

A Codex delegated implementation task stalled after Codex reported `patch rejected: writing outside of the project; rejected by user approval settings`. Agent Bridge kept the task `running` until manual stop, which hid an unrecoverable provider failure and forced the caller to infer state from raw logs.

## What Changes

- Reproduce the Codex sandbox patch-rejection failure with deterministic tests and captured diagnostics.
- Ensure unrecoverable Codex provider errors finalize tasks as failed instead of remaining indefinitely active.
- Classify Codex sandbox/approval denials with actionable `errorType`, diagnostics, logs, and `reviewPacket` guidance.
- Improve Codex provider command/prompt/runbook behavior if investigation shows Agent Bridge is causing out-of-workspace patch attempts.
- Preserve public MCP tool names and response compatibility; changes are additive or stricter failure finalization only.

## Capabilities

### New Capabilities
- `codex-provider-reliability`: Codex task execution failure handling, sandbox/approval diagnostics, and non-stalling lifecycle behavior.

### Modified Capabilities
- `rust-single-binary-mcp`: Task lifecycle must finalize provider processes that emit unrecoverable sandbox or approval errors.
- `delegated-review-packet`: Failed Codex tasks must give callers recovery guidance based on diagnostics without claiming verification.
- `mcp-usage-guidance`: Guidance must document how to investigate Codex sandbox/approval denials and avoid unsafe retries.

## Impact

- Affected code: `crates/agent-bridge-mcp/src/provider.rs`, `crates/agent-bridge-mcp/src/task.rs`, `crates/agent-bridge-mcp/src/guidance.rs`, stdio tests.
- Affected API: no new public MCP tools; `task_result`, `task_wait`, and `task_status` may expose more specific diagnostics for failed Codex tasks.
- Affected systems: Codex provider adapter, task lifecycle actor, provider log capture, operator guidance.
