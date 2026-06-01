## Context

After installing the updated bridge and reloading Codex MCP tools, `providers_check` without smoke succeeded for all providers: Claude through `claude-p`, Cursor through `cursor-agent`, Kimi through `pi`, and Codex through `codex`.

The first live `providers_check(smoke: true, timeoutMs: 30000)` exceeded Codex's MCP tool-call timeout because the bridge checks providers sequentially. A second run with `timeoutMs: 10000` completed but marked every provider unavailable with `provider_timeout`.

Direct task lifecycle probes with the same smoke phrase told a different story:

| Provider | Normal task result | Duration |
| --- | --- | ---: |
| codex | succeeded | ~11.5s |
| claude | succeeded | ~20.5s |
| kimi | succeeded | ~33.2s |
| cursor | succeeded | ~52.1s |

This means the current smoke failures are mostly a readiness-check contract problem. The bridge is using a fixed per-provider timeout, a sequential all-provider loop, and a full rendered task prompt for what operators expect to be a bounded health check.

## Goals / Non-Goals

**Goals:**

- Make provider readiness checks representative of real task-path viability.
- Let operators smoke one provider or a chosen subset without always running every provider.
- Keep `providers_check` responses bounded under Codex's MCP tool-call timeout.
- Distinguish version availability, auth/config/startup failures, smoke timeouts, and successful task-path readiness.
- Preserve deterministic fake-provider tests and avoid mandatory live provider execution in CI.
- Provide enough timing data to tune provider budgets over time.

**Non-Goals:**

- Do not require every provider to finish in the same fixed timeout.
- Do not make live model calls part of default automated tests.
- Do not silently mark a slow timeout as success.
- Do not replace task lifecycle tools with a separate long-running readiness job in this change.
- Do not add new provider CLIs or remove existing providers.

## Decisions

### Decision 1: Add provider filtering to `providers_check`

`providers_check` should accept an optional provider filter so operators can run `providers_check(smoke: true, providers: ["cursor"])` when investigating a slow or flaky provider. The all-provider default remains available for quick version checks and broader smoke runs.

Alternative considered: tell operators to use `task_spawn` directly for provider-specific smoke. That already works but bypasses `providers_check` diagnostics and keeps readiness troubleshooting scattered across lifecycle tools.

### Decision 2: Add aggregate host budget without changing `timeoutMs`

The current `timeoutMs` behaves as a per-provider timeout. That becomes dangerous when multiplied by four providers inside one MCP call, but silently changing its meaning would break existing callers. The new contract should add `aggregateTimeoutMs` for the entire `providers_check` call and keep `timeoutMs` as a per-provider fallback for callers that already use it.

Recommended additive inputs:

```json
{
  "smoke": true,
  "providers": ["claude", "codex"],
  "aggregateTimeoutMs": 110000,
  "providerTimeoutMs": {
    "claude": 30000,
    "cursor": 60000,
    "kimi": 45000,
    "codex": 20000
  }
}
```

`aggregateTimeoutMs` is the aggregate deadline for the whole `providers_check` call. If omitted, the aggregate default is 110000ms. That default leaves room for the default two-batch smoke critical path of cursor/kimi plus claude/codex and normal version-check overhead while staying below Codex's observed 120s MCP tool-call ceiling. `timeoutMs` remains a per-provider fallback when `providerTimeoutMs` omits a selected provider. If both `timeoutMs` and a provider-specific timeout are absent, provider-specific defaults are:

| Provider | Default smoke budget |
| --- | ---: |
| codex | 20000ms |
| claude | 30000ms |
| kimi | 45000ms |
| cursor | 60000ms |

If `providerTimeoutMs` omits a selected provider, that provider uses its default budget. If the aggregate deadline has less remaining time than a provider's budget, the provider receives only the remaining aggregate time.

Timeout input validation should reject non-integer, zero, negative, and unreasonably large timeout values. Initial maximums should be 120000ms for aggregate timeout and 90000ms for any single provider budget.

Alternative considered: only raise the default timeout. That helps today but does not solve aggregate host timeouts or slow-provider isolation.

### Decision 3: Run smoke probes concurrently with bounded collection

Version checks and smoke probes should both run under bounded deadlines. Version checks should use a short per-provider budget and the same process-group or child-tree cleanup behavior as smoke probes so a hung `--version` process cannot consume the entire MCP host timeout. After version checks succeed, smoke probes should run concurrently and collect results until the aggregate budget expires. Concurrency should be capped at two provider smoke probes at a time to avoid local resource spikes while still making all-provider smoke bounded by batches instead of the sum of all selected provider durations. The default can be overridden by `AGENT_BRIDGE_SMOKE_CONCURRENCY`: integer values greater than 4 clamp to 4, and empty, non-integer, zero, or negative values should be treated as unset and fall back to the default of 2.

Implementation must use explicit child-process cancellation. Each version or smoke probe should run in an isolated process group on Unix so timeout cleanup can terminate the provider CLI and any subprocesses it spawned. On non-Unix platforms, the implementation should use the platform-equivalent child tree cleanup primitive where available, or keep the current single-child behavior behind a documented limitation until a Windows job-object implementation exists. A probe future that is cancelled or times out must terminate the process group or child tree and await child exit before returning. Dropping a `tokio::process::Child` handle is not acceptable because it can leave provider processes running. Cancellation and deadline events should be logged to stderr with provider name, phase, elapsed time, and failure category while preserving stdout for MCP JSON-RPC only.

Alternative considered: keep sequential order and require users to filter one provider at a time. Filtering is useful, but all-provider smoke should still be practical.

### Decision 4: Add readiness phases and timing fields

Provider responses should make readiness state explicit. `startupVerified: true` should mean a smoke probe successfully completed on the task execution path. Failures should keep existing `diagnostic.failureCategory` values and add top-level elapsed timing fields for version and smoke probes: `versionDurationMs` and `smokeDurationMs`. Version-only checks include `versionDurationMs` and omit `smokeDurationMs` because no smoke phase ran.

The useful states are:

- `version`: binary exists and answered `--version`; task readiness unknown.
- `version+smoke`: binary exists and smoke was attempted.
- `startupVerified: true`: task-path smoke succeeded.
- `startupVerified: false` with diagnostic: smoke attempted but failed or timed out.

Alternative considered: add a new top-level readiness enum only. That may be clearer long-term, but keeping `startupVerified` stable and adding timing/diagnostic fields is less disruptive.

### Decision 5: Make smoke prompts intentionally minimal and provider-aware

Smoke should test "can this provider execute one bounded non-mutating prompt and return parseable output", not "can this provider process the bridge's full normal task instruction template quickly". The bridge should introduce a smoke prompt rendering mode that omits title, mode description, final-report instructions, and unrelated workflow text.

Initial provider mapping:

| Provider | Smoke prompt transport | Acceptance |
| --- | --- | --- |
| claude | same Claude stdin transport and output parser, minimal prompt only | parseable result contains `AGENT_BRIDGE_PROVIDER_SMOKE_OK` |
| cursor | same `cursor-agent` prompt argument path, minimal prompt only | process exits successfully and output contains `AGENT_BRIDGE_PROVIDER_SMOKE_OK` |
| kimi | same `pi -p` prompt argument path, minimal prompt only | process exits successfully and output contains `AGENT_BRIDGE_PROVIDER_SMOKE_OK` |
| codex | same `codex exec --json` path, minimal prompt only | JSON stream final output contains `AGENT_BRIDGE_PROVIDER_SMOKE_OK` |

If a provider cannot support the minimal smoke renderer safely, it must fall back to the standard task prompt and report that choice through diagnostics or timing so slow startup remains explainable. Implementation should model this as a provider smoke strategy, for example a small enum or trait method that returns the command prompt mode and expected acceptance parser for each provider. The default strategy should be minimal prompt plus the provider's existing output parser; fallback to standard task prompt should be explicit per provider rather than inferred from a runtime failure.

Alternative considered: continue using the full rendered task prompt. That is representative but too slow and noisy for readiness checks, especially for Codex because it may load skills/context before answering.

### Decision 6: Preserve task lifecycle as the fallback diagnostic path

If a provider is slow or ambiguous, the runbook should direct operators to spawn an explicit read-only task and inspect `task_status`, `task_logs`, and `task_result`. The investigation showed this path gave better evidence than a single all-provider smoke call.

Alternative considered: make `providers_check` spawn tracked tasks. That would add cleanup and state-management complexity to a tool intended to be a bounded probe.

## Risks / Trade-offs

- Concurrent smoke probes increase local resource usage -> Limit smoke concurrency to two selected providers and keep aggregate timeout enforcement.
- Provider-specific budgets can become stale -> Return elapsed timings so defaults can be tuned from real runs.
- Minimal smoke prompt misses a provider failure that full tasks hit -> Keep smoke scoped to readiness and document task lifecycle probes as deeper diagnostics.
- Additive input shape becomes too complex -> Keep defaults simple; advanced fields are optional and only needed for troubleshooting.
- Concurrency needs emergency disablement -> Support `AGENT_BRIDGE_SMOKE_CONCURRENCY=1` as a sequential fallback without changing API shape.
- Some providers need long first-run initialization -> Classify timeout clearly and let operators rerun a single provider with a larger provider budget.

## Migration Plan

1. Add fake-provider tests for the current all-provider sequential timeout failure shape, slow-but-successful smoke probes, per-provider filters, aggregate timeouts, concurrent collection, and child process-group reaping on timeout. Run the new characterization tests against the current implementation first and confirm the expected failure before changing provider readiness code.
2. Add `providers_check` input parsing and MCP tool schema entries for optional provider filters, additive aggregate timeout, and provider-specific budgets.
3. Refactor smoke execution to run selected provider probes concurrently under an aggregate deadline.
4. Add provider-aware minimal smoke rendering where needed while preserving provider binary/env/cwd/output parsing paths.
5. Add timing fields and diagnostics for version and smoke phases.
6. Update README live-smoke guidance with provider filtering, budget examples, and fallback task lifecycle probes.
7. Run `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, and OpenSpec validation.

Rollback: keep version checks and task lifecycle unchanged; if concurrent smoke causes issues, set `AGENT_BRIDGE_SMOKE_CONCURRENCY=1` while retaining provider filtering and clearer diagnostics.

## Open Questions

- Should slow providers such as Cursor eventually get a provider-specific first-run cache warmup hint, or is timing-based diagnostics enough for v1?
