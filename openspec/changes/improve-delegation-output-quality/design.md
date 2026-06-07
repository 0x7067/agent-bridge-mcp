## Context

`task.rs` is 3,308 lines—27% of the crate—and mixes six responsibilities: spawn orchestration, child process supervision, transcript bookkeeping, completion classification, registry persistence, and review-packet assembly. Cognitive-complexity and too-many-lines warnings are inevitable here. The failure taxonomy is stringly-typed: `failure_category` appears as `Option<&'static str>` in `ProbeResult`, `Option<String>` in `HostRunResult`, and raw strings in `provider_diagnostic` and `agent_diagnostic`. Typos cannot be caught at compile time.

Quality gating is provider-heterogeneous. `ClaudeAdapter` enforces `output_is_acceptable` (JSON parseability with non-empty `result`); all other adapters accept exit 0 blindly. Codex has `fatal_denial` detection but only for failure classification, not success validation. There is no retry mechanism: a `provider_timeout` or `provider_start_error` yields a terminal failure, forcing the caller to reconstruct the entire `agent_spawn` request. Partial results—provider output collected before a crash—exist in `transcript.jsonl` but are never surfaced in `agent_result` or the `next` action list.

## Goals / Non-Goals

**Goals:**
1. Enumerate every failure category as a strongly typed enum with stable serialization, replacing all string occurrences.
2. Require every `ProviderAdapter` to implement a uniform `acceptance_report` check so no provider is trusted on exit 0 alone.
3. Add bounded retry to `agent_spawn` for transient failures, controlled by an optional caller-supplied policy.
4. Surface partial results in `agent_result` and guide the caller toward continuation via the `next` list.
5. Split `task.rs` into five cohesive submodules aligned with its natural responsibilities.

**Non-Goals:**
- Changing the MCP protocol version or adding new protocol-level methods.
- Altering the `ProviderAdapter` trait surface beyond the mandated `acceptance_report` method.
- Adding circuit breakers, rate limiting, or queue scheduling (separate architectural concerns).
- Rewriting the Claude host runner or PTY interactive subsystem.

## Decisions

### D1: Enum-first failure taxonomy with dedicated serialize/deserialize
**Decision:** Introduce a `FailureCategory` enum with variants such as `ProviderTimeout`, `ProviderExitError`, `ProviderStartError`, `ProviderOutputError`, `ProviderSandboxDenied`, `HostRunnerUnavailable`, `WorktreeCleanupFailed`, `TranscriptUnavailable`, etc. Implement custom `Serialize`/`Deserialize` using kebab-case (e.g., `provider-timeout`).
**Rationale:** Guarantees exhaustiveness checking at compile time. Centralizes the string mapping in one conversion layer, eliminating typo risk in `provider_diagnostic`, `host_probe_result`, and the host-runner wire format.
**Trade-off:** Adding a variant requires a coordinated edit across all match arms. This is desirable because it forces explicit consideration of how new categories interact with diagnostics, retry logic, and dashboards.

### D2: AcceptanceReport as a trait method returning a structured verdict
**Decision:** Extend `ProviderAdapter` with `fn acceptance_report(&self, stdout: &[u8], stderr: &[u8]) -> AcceptanceReport`. The report contains `acceptable: bool`, `reason: Option<String>`, and `category: Option<FailureCategory>`.
**Rationale:** Makes output validation a contractual obligation for every provider, not an optional enhancement. Moves the “what does success look like?” question into the provider-specific module where the CLI contract is understood.
**Alternative considered:** Global heuristic scanner in `classify_completion` – rejected because it would accumulate provider-specific hacks centrally, violating the Adapter pattern's encapsulation intent.

### D3: Retry policy stored on the task record and evaluated in the actor
**Decision:** Add `retry_policy: Option<RetryPolicy>` to `TaskRecord` and evaluate retries inside `TaskActor::complete`. When a completion arrives with a transient category, the actor increments `attempt_count`, recomputes the backoff delay, and replays `spawn` with the same arguments if under budget.
**Rationale:** Keeping retry logic in the actor serializes it naturally with respect to the active-task limit and registry persistence. The caller merely expresses intent; the executor honors bounds.
**Trade-off:** The `agent_id` changes on each retry attempt because `spawn` mints a new UUID. To preserve continuity, the original `agent_id` is retained as `parent_agent_id` in the retried record.

### D4: Five-way split of `task.rs`
**Decision:** Decompose into:
- `task/spawn.rs` – argument validation, worktree creation, command building, and `launch_task` dispatch.
- `task/supervise.rs` – child process supervision, signal handling, timeout management, and the `wait_for_child` select loop.
- `task/complete.rs` – `classify_completion`, `host_completion`, `codex_denial_completion`, git snapshots, and `TaskCompletion` builders.
- `task/registry.rs` – `Registry` struct, `load_registry`, `save_registry`, `normalize_legacy_registry_fields`, and the startup stale-task reconciliation sweep.
- `task/review.rs` – `public_task`, `review_packet`, `agent_progress`, `next_actions`, `transcript_evidence`, and all presentation-layer helpers.
**Rationale:** Each submodule exports a narrow interface consumed by `TaskActor` in `task.rs`, which collapses to a thin coordinator. Compilation units shrink, test mocking becomes tractable, and cognitive complexity per file drops below 500 lines.
**Alternative considered:** Six or seven smaller modules – rejected because it fragments the natural transaction boundary between spawn and supervise. Five aligns with the lifecycle states (pending → active → done → persisted → presented).

### D5: Partial results extracted from transcript tail
**Decision:** During `classify_completion`, scan the trailing N lines of `transcript.jsonl` for `provider_result` or `provider_event` kinds. Populate `partial_results: Vec<PartialResult>` on the `TaskRecord` when the task is final but `final_result_detected` is false and `partial_result_detected` is true.
**Rationale:** The transcript is the ground truth; partial results are already there. Re-scanning the tail is cheap and avoids introducing a new sidecar file.
**Trade-off:** Very large transcripts may incur a modest seek cost. Bounding the scan to the last 1,024 lines caps this.

## Risks / Trade-offs

- **[Risk]** Refactoring `task.rs` into five modules while simultaneously changing the failure taxonomy and adapter traits may create a wide blast radius in a single PR.
  → Mitigation: Sequence the work. (1) Typed taxonomy lands first as a pure refactor with no behavioral change. (2) Modularization follows. (3) Acceptance reports and retry land last.
- **[Risk]** Stricter output validation may retroactively fail tasks that previously succeeded on exit 0 with empty or malformed stdout.
  → Mitigation: Initially gate the universal validator behind a `strict_validation` config flag (default false). Flip the default after a bake-in period.
- **[Risk]** Retry loops interacting with the active-task limit could starve other tasks if backoff is too short.
  → Mitigation: Retries count against the active limit. The actor computes jittered exponential backoff (min 1 s, max 30 s) and skips the retry if the limit is saturated.
- **[Risk]** Scanning transcript tails for partial results during the `complete` callback delays finalization.
  → Mitigation: The scan is bounded and performed asynchronously in a spawned task if latency becomes observable.

## Migration Plan

1. **Phase 0 (taxonomy)** — Replace all string failure categories with the enum. Zero functional change.
2. **Phase 1 (modularization)** — Mechanical extraction into `spawn.rs`, `supervise.rs`, `complete.rs`, `registry.rs`, `review.rs`. Run `cargo test` after every extraction to bisect breakage.
3. **Phase 2 (acceptance)** — Add `acceptance_report` to the trait; default implementation returns `acceptable: true` to preserve existing behavior. Override per provider.
4. **Phase 3 (retry)** — Add `RetryPolicy` deserialization, actor evaluation, and `agent_spawn` schema update.
5. **Phase 4 (partial results)** — Wire transcript scanning into `complete.rs` and `review.rs`.
6. **Phase 5 (enable strict mode)** — Toggle `strict_validation` default to true after observing green CI and dogfood workloads.

## Open Questions

- Should the typed taxonomy also subsume `ErrorType` (`Timeout`, `ProviderExitError`, etc.) or coexist with it? Recommendation: unify them; `ErrorType` is already a restricted subset and the distinction adds confusion.
- Should `acceptance_report` operate on raw bytes or parsed lines? Recommendation: raw bytes with a default UTF-8 lossy conversion, letting adapters decide whether to parse JSON or plaintext.
- Is a `RetryPolicy` per-spawn sufficient, or should there be a server-wide default in `Config`? Recommendation: both; caller policy overrides server default.
