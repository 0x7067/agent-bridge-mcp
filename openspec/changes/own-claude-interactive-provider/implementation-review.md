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
