## 1. Typed Failure Taxonomy

- [x] 1.1 Define `FailureCategory` enum with all known variants and custom `Serialize`/`Deserialize` using kebab-case
- [x] 1.2 Replace `failure_category: Option<&'static str>` in `ProbeResult` with `Option<FailureCategory>`
- [x] 1.3 Replace `failure_category: Option<String>` in `HostRunResult` with `Option<FailureCategory>`
- [x] 1.4 Update `provider_diagnostic`, `agent_diagnostic`, and `host_probe_result` to serialize the enum at the JSON boundary
- [x] 1.5 Audit `task.rs`, `server.rs`, `provider.rs`, and `claude_host.rs` for any remaining string literals representing failure categories and replace them
- [x] 1.6 Add unit tests proving round-trip serialization for every variant
- [x] 1.7 Run `scripts/quality.sh` and verify no new warnings or machete hits

## 2. Modularize task.rs

- [x] 2.1 Create `task/spawn.rs` and move `validate_spawn_arguments`, `create_worktree`, `launch_task`, `apply_launch_outcome`, and `run_host_task` into it
- [x] 2.2 Create `task/supervise.rs` and move `wait_for_child`, `ChildIoDrains`, `configure_child_process_group`, and the `tokio::select!` timeout loop into it
- [x] 2.3 Create `task/complete.rs` and move `classify_completion`, `classify_success_exit`, `classify_failure_exit`, `codex_denial_completion`, `host_completion`, and `agent_diagnostic` into it
- [x] 2.4 Create `task/registry.rs` and move `Registry`, `load_registry`, `save_registry`, `parse_registry_text`, `normalize_legacy_registry_fields`, and `cleanup_registry_temps` into it
- [x] 2.5 Create `task/review.rs` and move `public_task`, `review_packet`, `agent_progress`, `next_actions`, `transcript_evidence`, `insert_evidence_fields`, and `insert_detail_fields` into it
- [x] 2.6 Reduce `task.rs` to a thin `TaskActor` dispatcher that imports from the five submodules
- [x] 2.7 Run `cargo test` after each extraction to isolate breakage; ensure `server_protocol.rs` tests still pass
- [x] 2.8 Run `scripts/quality.sh` and verify no new warnings or machete hits

## 3. Universal Output Validation

- [x] 3.1 Add `acceptance_report` and `acceptance_criteria` defaulted methods to `ProviderAdapter` trait in `provider.rs`
- [x] 3.2 Implement `acceptance_report` for `ClaudeAdapter` migrating the existing `claude_output_is_parseable` check
- [x] 3.3 Implement `acceptance_report` for `CodexAdapter` incorporating `codex_denial_text` into the acceptance logic
- [x] 3.4 Implement `acceptance_report` for `CursorAdapter` (initially permissive, documenting the gap)
- [x] 3.5 Implement `acceptance_report` for `KimiAdapter` (initially permissive, documenting the gap)
- [x] 3.6 Implement `acceptance_report` for `AntigravityAdapter` (initially permissive, documenting the gap)
- [x] 3.7 Integrate `acceptance_report` into `classify_success_exit` in `task/complete.rs`
- [x] 3.8 Add `strict_validation` bool to `Config` (default false) and gate the universal acceptance check behind it
- [x] 3.9 Add unit tests for `AcceptanceReport` permutations (acceptable true/false, with/without reason)
- [x] 3.10 Run `scripts/quality.sh` and verify no new warnings or machete hits

## 4. Auto Retry Policy

- [x] 4.1 Add `RetryPolicy` struct with `max_retries` and `backoff_ms` to `domain.rs` or `task.rs`
- [x] 4.2 Extend `agent_spawn` input schema in `tools.rs` to accept optional `retryPolicy`
- [x] 4.3 Store `retry_policy` and `attempt_count` on `TaskRecord`
- [x] 4.4 Define `Transient` vs `Permanent` categorization on `FailureCategory` variants
- [x] 4.5 Modify `TaskActor::complete` to evaluate transient failures and schedule a respawn if budget permits
- [x] 4.6 Compute jittered exponential backoff with clamp at 30 seconds and minimum 1 second
- [x] 4.7 Append `retry_attempt` transcript events before each respawn
- [x] 4.8 Add integration test asserting retry exhaustion, backoff delay, and non-retry for permanent failures
- [x] 4.9 Run `scripts/quality.sh` and verify no new warnings or machete hits

## 5. Partial Result Surfacing

- [x] 5.1 Implement `scan_partial_results` in `task/complete.rs` that reads the last 1,024 lines of `transcript.jsonl` for `provider_event` entries without a `provider_result`
- [x] 5.2 Populate `partial_results: Vec<PartialResult>` on `TaskRecord` during finalization when `partial_result_detected` is true and `final_result_detected` is false
- [x] 5.3 Include `partialResults` in `agent_result` payload in `task/review.rs`
- [x] 5.4 Update `next_actions` in `task/review.rs` to suggest continuation/rerun when `partialResults` is non-empty
- [x] 5.5 Add unit tests for partial result scanning with fixtures: (a) final result dominates, (b) partial result emerges, (c) no result at all
- [x] 5.6 Run `scripts/quality.sh` and verify no new warnings or machete hits

## 6. Regression & Enablement

- [ ] 6.1 Run full test suite: `cargo test -- --test-threads=1`
- [ ] 6.2 Run hard gates: `scripts/quality.sh`
- [ ] 6.3 Update `CHANGELOG.md` or release notes documenting the new quality gates, retry policy, and modularization
- [ ] 6.4 Update `docs/agents/` with behavior differences for provider operators
- [ ] 6.5 After bake-in period, flip `strict_validation` default to `true` in a follow-up commit
- [ ] 6.6 Archive the change via `openspec archive` when all tasks are ticked
