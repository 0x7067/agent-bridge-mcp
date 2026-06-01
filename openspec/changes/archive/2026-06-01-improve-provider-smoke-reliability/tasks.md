## 1. Characterization And Fixtures

- [x] 1.1 Add a characterization test that reproduces the current all-provider sequential smoke timeout shape before implementation changes.
- [x] 1.2 Run the new characterization test against the current provider readiness implementation and confirm it fails for the expected timeout behavior before making it pass.
- [x] 1.3 Add fake-provider coverage for slow-but-successful smoke probes that exceed the old 10s-style timeout but complete inside a provider-specific budget.
- [x] 1.4 Add fake-provider coverage for provider-specific timeout diagnostics when a smoke probe exceeds its budget.
- [x] 1.5 Add fake-provider coverage for all-provider smoke completing under an aggregate timeout with elapsed wall-clock time below the sum of individual provider sleeps.
- [x] 1.6 Add validation coverage for invalid provider filters.
- [x] 1.7 Add validation coverage for empty provider filters and duplicate provider entries.
- [x] 1.8 Add validation coverage for invalid `aggregateTimeoutMs` and invalid `providerTimeoutMs` entries.
- [x] 1.9 Add timeout coverage proving unfinished smoke process groups or provider child trees are killed and reaped.
- [x] 1.10 Add coverage proving `timeoutMs` remains a per-provider fallback and is not reinterpreted as aggregate timeout.
- [x] 1.11 Add coverage for `AGENT_BRIDGE_SMOKE_CONCURRENCY=1` sequential fallback.
- [x] 1.12 Add coverage for invalid `AGENT_BRIDGE_SMOKE_CONCURRENCY` values falling back to the default cap and values greater than 4 clamping to 4.
- [x] 1.13 Add aggregate-timeout coverage with two in-flight fake providers proving both are terminated and reaped.
- [x] 1.14 Add coverage for provider filtering with `smoke: false` and verify unselected providers are omitted from the response array.
- [x] 1.15 Add timeout coverage proving a hung version probe is killed and reaped.
- [x] 1.16 Add validation coverage for unknown `providerTimeoutMs` keys.

## 2. Providers Check API

- [x] 2.1 Extend `providers_check` input parsing with an optional provider filter.
- [x] 2.2 Extend `providers_check` input parsing with additive `aggregateTimeoutMs`.
- [x] 2.3 Extend `providers_check` input parsing with optional provider-specific smoke budgets.
- [x] 2.4 Define and apply default budgets: aggregate 110000ms, codex 20000ms, claude 30000ms, kimi 45000ms, cursor 60000ms.
- [x] 2.5 Keep existing `timeoutMs` behavior as a per-provider fallback.
- [x] 2.6 Preserve existing `providers_check` calls without new fields.
- [x] 2.7 Return top-level `versionDurationMs` for version checks and `smokeDurationMs` only when a smoke phase runs.
- [x] 2.8 Update the MCP tool input schema to advertise `providers`, `aggregateTimeoutMs`, and `providerTimeoutMs` as an object keyed by supported provider names with integer millisecond values from 1 through 90000.

## 3. Smoke Execution

- [x] 3.1 Refactor smoke execution to run selected provider probes concurrently under an aggregate budget with maximum concurrency of two.
- [x] 3.2 Terminate and reap unfinished smoke process groups or provider child trees when their provider-specific budget or aggregate budget expires.
- [x] 3.3 Apply bounded version-check timeouts with the same process-group or provider child-tree cleanup path.
- [x] 3.4 Keep deterministic diagnostics for `provider_timeout`, `provider_start_error`, `provider_exit_error`, and `provider_output_error`.
- [x] 3.5 Add an explicit provider smoke strategy abstraction for minimal versus standard task prompt rendering, preserving binary, environment, cwd, and output parsing paths.
- [x] 3.6 Report smoke prompt strategy when falling back to the standard task prompt.
- [x] 3.7 Emit bounded stderr diagnostics for smoke deadline and cancellation events without writing non-MCP bytes to stdout.

## 4. Documentation

- [x] 4.1 Update README live-smoke examples to show provider filtering.
- [x] 4.2 Document provider-specific budget guidance using the measured live durations from this investigation.
- [x] 4.3 Document the fallback task lifecycle probe for slow or ambiguous providers.

## 5. Verification

- [x] 5.1 Run `cargo fmt --check`.
- [x] 5.2 Run `cargo test`.
- [x] 5.3 Run `cargo clippy --all-targets -- -D warnings`.
- [x] 5.4 Run `openspec validate improve-provider-smoke-reliability`.
