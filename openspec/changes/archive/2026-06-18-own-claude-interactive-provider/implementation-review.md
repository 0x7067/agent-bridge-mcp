## Agent Bridge Review Log

## 2026-06-05 Cursor Review

Reviewer: Agent Bridge Cursor, mode `review`

Task id: `task_a1acfef7d6a04c5f980f2ea98f7858d1`

Verdict: `NOT_READY` for implementation tasks beyond research.

Blocking findings recorded from the review:

1. Host-runner protocol v2 schema was not concrete enough.
2. Runner-result integration did not define how structured v2 output replaces
   print-mode JSON stdout parsing.
3. PTY adapter choice needed a runtime spike before production wiring.
4. Startup sequencing needed to specify quiescence, SessionStart, prompt write,
   and separate Enter behavior.
5. Hook relay needed explicit env names, temporary settings, FIFO ordering, and
   StopFailure registration.
6. StopFailure input values needed to use canonical installed Claude values.
7. Login-shell bootstrap needed to avoid arbitrary shell command protocol while
   preserving local Claude auth/PATH behavior.

Actions taken in this revision:

- Added `protocol-v2.md`.
- Added `runner-result-contract.md`.
- Added `hook-relay-contract.md`.
- Added `startup-sequencing.md`.
- Updated design/tasks/spec requirements to reference the concrete contracts.
- Canonicalized StopFailure input values to installed Claude Code 2.1.165
  values from `claude-stopfailure-setup-signatures.md`.
- Added a required PTY adapter spike before production runner wiring.

Implementation beyond research remains gated on a follow-up Agent Bridge review
of this revised plan.

## 2026-06-05 Cursor Re-Review

Reviewer: Agent Bridge Cursor, mode `review`

Task id: `task_9f8b43cf02d24cf383e5a3bf8082078f`

Verdict: `READY_WITH_P1`.

Review summary:

- The prior P0 blockers are closed enough to proceed with task 1.7.
- Implementation tasks remain gated on task 1.7 proving the selected PTY adapter
  behavior.
- The remaining risks are non-blocking and should be tracked during
  implementation: runtime validation of `pty-process`, nested diagnostic schema
  detail, transcript JSONL record selection, startup constant tuning, hook helper
  artifact shape, dual Stop/StopFailure precedence, and minor wording drift.

Action taken from the review:

- Corrected the remaining host-runner spec wording that still referred to
  captured stdout/stderr as the owned-runner result shape.
