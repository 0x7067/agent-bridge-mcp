## 1. Diagnosis

- [x] 1.1 Identify the race condition in `task/supervision.rs` where `exit_code == 0` masks `timed_out == true`.
- [x] 1.2 Measure blast radius: confirm the flaw affects `classify_completion` and downstream review packets.

## 2. Classification Logic

- [x] 2.1 Reorder `task/complete.rs` predicates so `timed_out` is evaluated before `exit_code`.
- [x] 2.2 Capture pre-deadline exit timestamp to protect legitimate successes from scheduler jitter.
- [x] 2.3 Map timeout outcomes to `FailureCategory::TimedOut` with a structured diagnostic.

## 3. Supervision Wiring

- [x] 3.1 Thread the `deadline_instant` from `task/supervision.rs` into the completion classifier.
- [x] 3.2 Ensure `tokio::select!` branches still drain IO and reap PIDs correctly after the precedence fix.

## 4. Testing

- [x] 4.1 Add unit test for late exit(0) after timeout → asserts `TimedOut`.
- [x] 4.2 Add unit test for near-deadline clean exit → asserts `Succeeded`.
- [x] 4.3 Run full `cargo test` and `scripts/quality.sh` to confirm no collateral damage.

## 5. Documentation

- [x] 5.1 Update `docs/agents/definition-of-done.md` if timeout behavior influences gate descriptions.
- [x] 5.2 Record the fix in `docs/ADR/` or changelog for archaeological reference.
