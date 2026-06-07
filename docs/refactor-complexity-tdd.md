# Complexity Refactor Plan (strict TDD)

## Context

The `quality` workflow runs complexity hotspots (clippy `cognitive_complexity` /
`too_many_lines` / `too_many_arguments`) as **reporting only**, because the tree
has ~14 source-level hotspots that would fail a `-D warnings` gate. This plan
drives those source hotspots below threshold so the complexity lints can be
promoted to a **hard gate**. Test-only hotspots in `tests/stdio_binary.rs` are a
separate, lower-priority follow-up.

Safety net: 61 `#[test]`/`#[tokio::test]` functions in `tests/stdio_binary.rs`
(end-to-end via the stdio binary), plus ~10 unit tests in `task.rs` and a handful
in `runner.rs`. **Caveat (from adversarial review):** the async hotspots
(`run_interactive`, `wait_for_child`) are covered **only** by the slow PTY
integration tests — there are *no* fast unit tests exercising their timeout/kill
paths today. And per MEMORY.md (`agent-bridge-flaky-pty-parallel-tests`) PTY
tests cross-kill under parallel load, so the suite must run **`--test-threads=1`**
while refactoring or red bars will be noise-flaked.

## Method — honest framing (revised after adversarial review)

This is **behavior-preserving refactoring guarded by tests**, not green-field TDD.
Calling a post-hoc "RED test" for code that already implements the behavior is
theatre — it passes the moment the helper compiles. So the cadence depends on the
seam type:

- **New PURE helper (string/JSON shaping, validation, classification):** genuine
  TDD applies. Write a failing unit test for the helper's contract over concrete
  inputs/outputs (it fails: helper absent), then extract minimally to green. These
  are fast, deterministic, and the bulk of the early work.
- **Extracting an async orchestration block (a `select!` loop):** there is no
  honest RED. Instead, **first add a characterization test** that pins the
  *current* observable output for the relevant cases (timeout → category, denial →
  category, success → transcript source). Confirm it is green on the unchanged
  code, extract, then confirm it stays green. The characterization test is the
  oracle, not a RED bar.

Per-step commands (note the single-threaded flag):

```
cargo test -p agent-bridge-mcp -- --test-threads=1
cargo clippy --all-targets -- -W clippy::cognitive_complexity -W clippy::too_many_lines -W clippy::too_many_arguments
```

Each hotspot is its own atomic `refactor:` commit (Conventional Commits).

## Scope & order — by RISK, not severity (revised)

Adversarial review was unanimous: do **not** start with the two `select!` loops.
They are the highest-risk and lowest-confidence extractions, and two proposed
seams are unsound as written (see Risks below). Build the gate with low-risk pure
helpers first; tackle the async loops last, conservatively.

**Phase A — pure helpers (low risk, genuine TDD, do first):**

| Location | Symbol | Lines | Approach |
|---|---|---|---|
| `task.rs:272` | `result` | 112 | extract response/JSON shaping helpers |
| `server/diagnostics.rs:768` | (fn) | 129 | extract per-section builders *only if it adds clarity* (already cleanly structured — may leave as-is) |
| `provider.rs:258` | (fn) | 109 | extract sequential section builders |
| `claude_host.rs:347` | (fn) | 106 | extract sequential section builders |

**Phase B — dispatch splitting (low-moderate risk):**

| Location | Symbol | Lines | Cognitive | Approach |
|---|---|---|---|---|
| `server.rs:70` | `call_tool` | 121 | 29 | one handler fn per tool arm |
| `server.rs:326` | `run_probe` | 130 | — | split branches |
| `task.rs:527` | `Handler::spawn` | 127 | — | extract validation + command assembly |
| `task.rs:1133` | `complete_host_response` | 127 | — | split transcript update from response assembly |
| `task.rs:965` | `launch_task` | 112 | — | extract setup helpers |

**Phase C — async orchestration (highest risk, last, conservative):**

| Location | Symbol | Lines | Cognitive | Approach |
|---|---|---|---|---|
| `task.rs:1266` | `wait_for_child` | 197 | 32 | extract only the **post-loop** `classify_completion`; bundle 7 args into `WaitForChild`; consider `await_child_exit` **only** by *moving* `child` in (not `&mut`) |
| `runner.rs:97` | `run_interactive` | 235 | 48 | extract only the **post-loop** `resolve_final_result`; **leave the `select!` loop and the Stop-acceptance arm intact** |

## Risks flagged by adversarial review (Phase C constraints)

These are HIGH-severity and constrain the async work:

1. **`wait_for_child`: pinned-future aliasing.** `wait` is `tokio::pin!`-ed and
   reborrowed as `&mut wait` in multiple arms (task.rs ~1290/1294/1309/1313). An
   `await_child_exit(&mut child, …)` helper that threads `&mut wait` will **not
   compile** — the pin guarantee lives in the caller. Only cuttable by *moving*
   `child` into the helper and returning `ChildExit { output, timed_out,
   fatal_denial }`. If that proves awkward, extract **only** the post-loop
   `classify_completion` and leave the loop in place.
2. **`run_interactive`: the Stop-acceptance arm is not a seam.** The "accept Stop
   only once the transcript carries a response" logic (runner.rs ~173–199)
   combines an `await`, a `&mut stop` mutation, and the loop's break decision.
   Extracting it breaks encapsulation or changes timeout interplay. **Leave it in
   the loop.** `completion_payload` and `resolve_stop_result` are already helpers —
   that is enough.
3. **`drive_event_loop` extraction is discouraged.** It would hold `&mut session`
   and `&mut event_rx` across the whole loop and hide the nested
   `finish_if_child_exited` poll on the same receiver. Compiles, but fragile and
   low-value. **Do not extract the loop**; only the post-loop resolution.

## Concrete decomposition (constrained)

### `runner.rs::run_interactive` (235 → target < 100 via post-loop extraction only)

- Keep `prepare_run` extraction (setup is genuinely separable, no shared async
  state): temp dir, relay, settings, channel, env, `spawn_claude` (lines ~100–150).
- **Do not** extract the `select!` loop.
- `resolve_final_result(stop, stop_failure, session_start, failure_category) ->
  ResolvedRun` — the pure post-loop transcript/final-text resolution (lines
  ~286–334). Add a genuine **failing unit test first** (pure, no PTY): maps a
  `StopFailure` payload to its category; maps a successful stop to
  `final_text_source = "transcript"`.

If `prepare_run` + `resolve_final_result` alone don't drop it under 100 lines,
accept a scoped `#[allow(clippy::too_many_lines)]` on the loop body with a comment
rather than forcing an unsafe seam.

### `task.rs::wait_for_child` (197 → target < 100)

- Bundle the 7 params into `WaitForChild { … }` (clears `too_many_arguments`).
- `classify_completion(exit, &command, &agent_dir, fatal_denial) -> TaskCompletion`
  — the trailing match (~1360+). Characterization test first.
- `record_lifecycle(&command, &agent_dir, &exit, timed_out)` — transcript appends
  (~1331–1359).
- `await_child_exit` **only if** moving `child` in is clean (Risk 1); otherwise
  leave the loop in place and accept the remaining length via scoped `#[allow]`.

### Phase A/B hotspots — pure & dispatch

Each splits along an already-visible boundary; prefer **pure** helpers (JSON/string
shaping, validation, classification) that take a genuine failing unit test:
`call_tool`/`run_probe` → one handler per branch; `spawn`/`launch_task`/`result`
→ validation + command assembly + response shaping; `complete_host_response` →
transcript update vs. response assembly. For `diagnostics.rs`, only split if it
*improves* clarity — review judged it already cleanly structured.

## Final step — promote the gate (revised)

**A global `-D deny` is impossible** as originally written: the literal worst
offenders are *test* functions in `tests/stdio_binary.rs` (247/235/215 lines, cog
60/35/27), which the plan rightly does not refactor (splitting them harms test
isolation and risks PTY flakiness). They would keep tripping a deny-level lint.
Also note `cognitive_complexity` is a clippy *nursery* heuristic with an arbitrary
threshold that can shift between releases — over-tightening invites churn.

Realistic end-state (pick one; recommended first):

1. **Src-only deny + test opt-out (recommended).** Add
   `#![allow(clippy::cognitive_complexity, clippy::too_many_lines)]` to the
   integration test crate / `#[cfg(test)]` modules, then enable the complexity
   lints at deny level for `src/` via a `[lints.clippy]` table in `Cargo.toml`
   (so editors enforce them too) and in the CI clippy step. Set generous
   thresholds in `clippy.toml` (e.g. `too-many-lines-threshold = 120`) so honest
   functions aren't punished.
2. **Baseline / new-code-only.** Keep the lints at *warn* (current reporting
   behavior) but fail CI only when the warning count *increases* vs. a committed
   baseline. No existing code must change; only new complexity is gated.
3. **Stay report-only.** If the churn/value trade-off isn't worth it, leave the
   complexity section informational (today's state) and stop after Phase A/B
   yields the easy wins.

## Verification

- `cargo test -p agent-bridge-mcp -- --test-threads=1` green after every commit
  (single-threaded to avoid the known PTY cross-kill flakiness).
- `bash scripts/quality.sh` — duplication/machete/fmt stay green; complexity
  section shrinks each commit until empty.
- Final: complexity lints at deny level pass with zero warnings.
