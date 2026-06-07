## Why

Provider output quality is inconsistently enforced. Today only Claude validates parseability; Cursor, Kimi, Codex, and Antigravity are accepted solely on exit code 0. The failure taxonomy is stringly-typed throughout diagnostics and the host-runner wire format, inviting drift and preventing programmatic reaction. There is no automated retry for transient failures, partial results from crashed tasks are invisible, and the task actor monolith (`task.rs`, 3,308 lines) concentrates spawn orchestration, IO drainage, completion classification, and registry persistence into one unmaintainable file. These gaps erode trust in delegated work and complicate debugging.

## What Changes

- Introduce a strongly typed `FailureCategory` enum mapped to kebab-case/snake-case strings at the JSON boundary, replacing raw `&'static str` and `Option<String>` fields everywhere.
- Require every `ProviderAdapter` to declare an `AcceptanceReport` describing whether produced output satisfies the provider's wire format. Fail the task if output is gibberish or empty despite exit 0.
- Add an optional `retryPolicy` argument to `agent_spawn` (e.g., `maxRetries`, `backoffMs`). The actor shall replay `spawn` for transient failures (`provider_timeout`, `provider_start_error`) up to the configured limit.
- Promote `partial_result_detected` into a first-class `agent_result` section and update the `next` action list to suggest continuation when partial results exist.
- Decompose `task.rs` into focused submodules: `task/spawn.rs`, `task/supervise.rs`, `task/complete.rs`, `task/registry.rs`, and `task/review.rs`.

## Capabilities

### New Capabilities
- `typed-failure-taxonomy`: Compile-time enumerated failure categories with stable string serialization.
- `universal-output-validation`: Mandatory provider adapter acceptance checks for every first-class provider.
- `auto-retry-policy`: Bounded retry with configurable backoff for transient spawn and execution failures.
- `partial-result-surfacing`: Exposure of incomplete but usable provider output when a task crashes or times out.

### Modified Capabilities
- `provider-adapter-contract`: Extends the adapter trait to require `acceptance_report()` and `acceptance_criteria()` methods.
- `task-run-transcripts`: Expands transcript evidence to include `partial_results` metadata.

## Impact

- Crate: `agent-bridge-mcp` (task, provider, server, domain)
- Dependencies: No new crates required; pure refactor plus incremental trait additions
- Public API: `agent_spawn` gains optional `retryPolicy` field; `agent_result` gains `partialResults` section
- Behavior: Strictly additive with higher quality bars; no existing valid flows break
