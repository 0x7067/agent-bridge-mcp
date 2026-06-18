# Release Notes

## Unreleased - Delegation Output Quality

- The local quality gate is `scripts/quality.sh`: rustfmt, clippy with `-D warnings`, cargo-machete, and jscpd under 5 percent duplication. Run `cargo test -p agent-bridge-mcp -- --test-threads=1` before release work that touches PTY or process lifecycle paths.
- Task lifecycle code is split into focused modules: `task/spawn.rs`, `task/supervision.rs`, `task/complete.rs`, `task/registry.rs`, and `task/review.rs`. `task.rs` remains the actor facade.
- Provider and task diagnostics now use a typed `FailureCategory` enum at the Rust boundary and stable `failureCategory` strings in JSON.
- `agent_spawn` accepts optional `retryPolicy` with `maxRetries` and `backoffMs`. Retries apply only to transient categories such as `provider_timeout`, `provider_start_error`, `claude_rate_limit`, `claude_model_unavailable`, `runner_timeout`, and `client_disconnected`.
- Final tasks can surface `partialResults` when transcript output exists but no final provider result was recorded.
- Provider success validation is adapter-owned through `acceptance_report` and `acceptance_criteria`. `strict_validation` defaults to false; when enabled, successful exits can still fail if provider output does not meet that adapter's contract. Codex sandbox or approval denial still fails even when strict validation is off.
