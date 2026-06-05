## 1. Provider Cadence Metadata

- [ ] 1.1 Add provider output-cadence metadata to `providers_list` for Claude, Cursor, Kimi, and Codex.
- [ ] 1.2 Encode Cursor JSON-mode as final-output-oriented with a conservative observation budget.
- [ ] 1.3 Add provider metadata tests proving cadence metadata is present and advisory.

## 2. Progress Model

- [ ] 2.1 Add a task progress summary derived from task status, timestamps, timeout, transcript events, and provider cadence metadata.
- [ ] 2.2 Track or derive `lastEventAt`, `lastOutputAt`, `silentForMs`, `elapsedMs`, `expectedOutputCadence`, `secondsUntilRecommendedCheck`, and `stallRisk`.
- [ ] 2.3 Ensure final tasks report no further polling needed and point callers to result inspection.
- [ ] 2.4 Add unit tests for no-output running tasks, first-output tasks, final tasks, and timeout-near tasks.

## 3. Observation Surface

- [ ] 3.1 Add a bounded `agent_observe` tool schema with `taskId`, `cursor`, `limit`, and `timeoutMs`.
- [ ] 3.2 Implement `agent_observe` as request/response long polling for new lifecycle or transcript events without server-pushed notifications.
- [ ] 3.3 Clamp observe timeout and event limit to documented maximums.
- [ ] 3.4 Return current task summary, events, `nextCursor`, observe-call `timedOut`, progress metadata, and next actions.
- [ ] 3.5 Add stdio tests for observe returning new events, observe timeout without provider failure, finalization, and clamping.

## 4. Next Actions And Presentation

- [ ] 4.1 Include progress metadata in running task presentation summaries.
- [ ] 4.2 Rank `agent_observe` or provider-aware `task_wait` ahead of `task_stop` while a task is within its recommended observation budget.
- [ ] 4.3 Mark stop/fallback reasons more explicitly when a task exceeds its expected silence budget or nears configured timeout.
- [ ] 4.4 Add tests for Cursor-style silent running tasks where stop is available but not primary.

## 5. Guidance And Documentation

- [ ] 5.1 Update initialization instructions, prompts, resources, and README to describe progress observation.
- [ ] 5.2 Document that Cursor JSON-mode can be silent until final output and should not be stopped solely because transcript shows only `spawned`.
- [ ] 5.3 Document fallback criteria based on final failure, provider timeout, explicit stop decision, or exceeded observation budget.

## 6. Verification

- [ ] 6.1 Run `cargo fmt --check`.
- [ ] 6.2 Run focused protocol and stdio tests for progress observation.
- [ ] 6.3 Run `cargo test -p agent-bridge-mcp`.
- [ ] 6.4 Run `openspec validate improve-task-progress-observation --strict`.
- [ ] 6.5 Run a live Cursor review smoke with a short observe loop and confirm the guidance does not recommend premature stop while the task is within budget.
