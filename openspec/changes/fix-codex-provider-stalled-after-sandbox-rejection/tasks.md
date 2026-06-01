## 1. Reproduction And Diagnosis

- [x] 1.1 Add a failing stdio regression test for a Codex task that exits after emitting `patch rejected` / outside-project / approval-denied stderr evidence.
- [x] 1.2 Add a failing stdio regression test for a Codex task that emits the same fatal denial evidence and then hangs until Agent Bridge terminates it.
- [x] 1.3 Add test assertions proving MCP stdout remains clean and stderr/log excerpts capture the denial without exposing the task prompt or secret-like environment values.
- [x] 1.4 Inspect the current Codex command descriptor from `task_preview` and document whether `--cd`, sandbox mode, prompt transport, or installed-binary drift contributed to the observed failure.

## 2. Lifecycle Finalization

- [x] 2.1 Implement Codex fatal-denial detection with a narrow, documented pattern for sandbox, approval, and out-of-project patch rejection evidence.
- [x] 2.2 Ensure fatal-denial detection transitions the task to final `failed` state through the normal task manager completion path.
- [x] 2.3 Ensure a hanging Codex process with fatal-denial evidence is terminated and reaped within a bounded cleanup deadline.
- [x] 2.4 Keep intended timeout semantics for providers that do not emit known fatal-denial evidence; do not preserve exact legacy response shapes unless they still express the intended contract.

## 3. Diagnostics And Review Packet

- [x] 3.1 Add a stable Codex sandbox/approval denial classification, using a dedicated `errorType`, `diagnostic.failureCategory`, or both if that is the clearest public contract.
- [x] 3.2 Include provider name, command path/kind, launch strategy, exit metadata, and redacted stdout/stderr excerpts in failed Codex diagnostics.
- [x] 3.3 Update `reviewPacket.recommendedActions` for Codex denial failures to direct callers to inspect logs, cwd/workspace policy, prompt scope, and isolation strategy.
- [x] 3.4 Add tests proving review packets do not recommend silently relaxing sandbox permissions or blindly retrying.

## 4. Codex Adapter Corrections

- [x] 4.1 If diagnosis identifies command-shape or prompt-scope issues, update the Codex provider adapter with focused tests for `task_preview` and `task_spawn`.
- [x] 4.2 If diagnosis does not identify adapter issues, document that the fix is lifecycle/diagnostic handling only; avoid command behavior churn, but do not keep behavior unchanged solely for compatibility.
- [x] 4.3 Verify normal successful Codex-like fake-provider tasks still succeed and that public lifecycle responses express the intended contract, even if old compatibility-only fields or categories change.

## 5. Guidance And Operator Runbook

- [x] 5.1 Add failing guidance tests proving recovery/safety/provider guidance mentions Codex patch rejection or sandbox/approval denial symptoms.
- [x] 5.2 Update MCP prompts/resources and README guidance with bounded recovery steps using `task_wait`, `task_logs`, `task_status`, and `task_result`.
- [x] 5.3 Ensure guidance tells callers to inspect `cwd`, workspace policy, prompt scope, and isolation strategy before retrying.

## 6. Verification

- [x] 6.1 Run focused stdio tests for Codex denial finalization, diagnostics, and review packet guidance.
- [x] 6.2 Run existing provider lifecycle stdio tests to prove non-Codex timeout, success, safety, and log/result semantics remain correct.
- [x] 6.3 Run `cargo fmt --check`.
- [x] 6.4 Run `cargo test`.
- [x] 6.5 Run `cargo clippy --all-targets -- -D warnings`.
- [x] 6.6 Run `openspec validate fix-codex-provider-stalled-after-sandbox-rejection`.
- [x] 6.7 Optionally run a live Codex dogfood task with a harmless prompt and document whether the original failure reproduces.
