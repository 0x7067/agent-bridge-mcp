## 1. Provider Cadence Metadata

- [x] 1.1 Add provider output-cadence metadata to `providers_list` for Claude, Cursor, Kimi, and Codex.
- [x] 1.2 Encode Cursor JSON-mode as final-output-oriented with a conservative observation budget.
- [x] 1.3 Add provider metadata tests proving cadence metadata is present and advisory.
- [x] 1.4 Document and test the exact output-cadence schema fields and advisory semantics.

## 2. Progress Model

- [x] 2.1 Add a task progress summary derived from task status, timestamps, timeout, transcript events, and provider cadence metadata.
- [x] 2.2 Track or derive `lastEventAt`, `lastOutputAt`, `silentForMs`, `elapsedMs`, `expectedOutputCadence`, `secondsUntilRecommendedCheck`, and `stallRisk`.
- [x] 2.3 Ensure final tasks report no further polling needed and point callers to result inspection.
- [x] 2.4 Add unit tests for no-output running tasks, first-output tasks, final tasks, and timeout-near tasks.
- [x] 2.5 Keep progress derivation bounded or cached so status/list calls do not require unbounded transcript rescans.

## 3. Observation Surface

- [x] 3.1 Add a bounded `agent_observe` tool schema with `taskId`, `cursor`, `limit`, and `timeoutMs`, and include it in the canonical `agent_*` tool inventory.
- [x] 3.2 Implement `agent_observe` as request/response long polling for new lifecycle or transcript events without server-pushed notifications.
- [x] 3.3 Clamp observe timeout and event limit to documented maximums.
- [x] 3.4 Return current task summary, events, `nextCursor`, observe-call `timedOut`, progress metadata, and next actions.
- [x] 3.5 Add stdio tests for observe returning new events, observe timeout without provider failure, finalization, and clamping.
- [x] 3.6 Add race tests for finalization during observe and stop during observe.

## 4. Next Actions And Presentation

- [x] 4.1 Include progress metadata in running task presentation summaries.
- [x] 4.2 Rank `agent_observe` or provider-aware `agent_wait` ahead of `agent_stop` while a task is within its recommended observation budget.
- [x] 4.3 Mark stop/fallback reasons more explicitly when a task exceeds its expected silence budget or nears configured timeout.
- [x] 4.4 Add tests for Cursor-style silent running tasks where stop is available but not primary.
- [x] 4.5 Add an `observe` entry to `presentation.actions` so UI clients see the new lifecycle control.
- [x] 4.6 Rename presentation action tool names and top-level `nextActions` from `task_*` to canonical `agent_*`.

## 5. Guidance And Documentation

- [x] 5.1 Update initialization instructions, prompts, resources, and README to describe progress observation through the canonical `agent_*` lifecycle.
- [x] 5.2 Document that Cursor JSON-mode can be silent until final output and should not be stopped solely because transcript shows only `spawned`.
- [x] 5.3 Document fallback criteria based on final failure, provider timeout, explicit stop decision, or exceeded observation budget.
- [x] 5.4 Replace short-wait-only stalled task guidance across initialization instructions, prompts, resources, README, and recommended actions.
- [x] 5.5 Remove mixed `agent_*`/`task_*` workflow guidance so clients see one public tool family.

## 6. Verification

- [x] 6.1 Run `cargo fmt --check`.
- [x] 6.2 Run focused protocol and stdio tests for progress observation.
- [x] 6.3 Run `cargo test -p agent-bridge-mcp`.
- [x] 6.4 Run `openspec validate improve-task-progress-observation --strict`.
- [x] 6.5 Run a live Cursor review smoke with a short observe loop and confirm the guidance does not recommend premature stop while the task is within budget.
