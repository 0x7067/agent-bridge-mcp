## Context

Agent Bridge already stores lifecycle events, raw logs, normalized transcripts, presentation summaries, and `nextActions`. The weak point is the supervision loop used by human callers and harnesses: a running provider task can look idle because some providers, notably Cursor with JSON output, emit no stdout until completion. In the recent review-gate incident, Cursor was stopped after roughly 80 seconds even though its configured timeout was 600 seconds and recent successful Cursor reviews commonly completed after 50-190 seconds.

MCP protocol-level task notifications are not portable across the target hosts yet. The current server already supports request/response lifecycle tools, so the near-term design should improve observation through additive polling metadata and an optional long-poll request rather than relying on server-pushed notifications.

## Goals / Non-Goals

**Goals:**
- Help callers distinguish healthy silence from a true stalled task.
- Give harnesses provider-aware polling intervals, stop/fallback thresholds, and ready-to-call observation actions.
- Preserve raw `task_logs`, bounded `task_transcript`, and final `task_result` as the source of evidence.
- Add an event-style observation path that can wait for new transcript/lifecycle activity or finalization without forcing clients to poll in a tight loop.
- Keep completion and verification boundaries unchanged.

**Non-Goals:**
- Do not implement MCP Tasks.
- Do not advertise `io.modelcontextprotocol/tasks`.
- Do not depend on server-pushed JSON-RPC notifications.
- Do not stream raw provider stdout/stderr unboundedly.
- Do not infer that a silent provider is correct or verified.

## Decisions

### Add `task_observe` rather than overloading `task_wait`

`task_wait` answers one narrow question: "is this final yet?" A richer observation surface needs to answer "what changed since cursor N, what should I do next, and how long should I wait before escalating?" A dedicated `task_observe` tool keeps those semantics explicit.

`task_observe` should accept:
- `taskId`
- `cursor` for transcript/lifecycle event position
- `timeoutMs` for bounded long polling
- `limit` for returned event count
- optional `includeLogs` or `includeTranscriptEvents` flags only if needed after initial implementation

The response should include:
- current public task summary and presentation
- `events` since the cursor, capped by `limit`
- `nextCursor`
- `timedOut` for the observe call, not the provider task
- `progress` metadata such as `elapsedMs`, `lastEventAt`, `lastOutputAt`, `secondsUntilRecommendedCheck`, `silentForMs`, `expectedOutputCadence`, and `stallRisk`
- updated `nextActions`

### Model output cadence as provider metadata

Provider adapters should expose output cadence guidance in `providers_list`, for example:

- Cursor: `final_json`, high first-output latency, recommend waiting near the task timeout before fallback unless the caller set a short timeout intentionally.
- Claude/Kimi/Codex: output cadence may be final-only or incremental depending on CLI mode, but the metadata should describe the observed adapter behavior rather than provider marketing.

This keeps harnesses from hardcoding Cursor-specific delays while still making the observed behavior discoverable.

### Use adaptive next actions for running tasks

Running task `nextActions` should prefer `task_observe` or `task_wait` with provider-aware arguments. For silent final-output providers, the primary action should be a longer bounded observe/wait rather than `task_stop`. Stop remains available but should be marked unsafe until the task is beyond the recommended observation budget or the caller has evidence that the task is no longer useful.

### Treat notifications as future compatibility

If a future host supports MCP notifications or task extensions, Agent Bridge can map the same observation events to that transport. This change should keep the internal event model and response shape compatible with a future stream, but the runtime API should remain request/response.

## Risks / Trade-offs

- [Risk] More metadata can look like certainty about provider health. -> Mitigation: use names like `stallRisk`, `expectedOutputCadence`, and `recommended`, and keep verification status separate.
- [Risk] Long-poll requests can tie up the task actor. -> Mitigation: implement observe without blocking the actor on child processes; use bounded waits and actor-friendly notifications or short sleep loops similar to `task_wait`.
- [Risk] Provider cadence defaults may drift as CLIs change. -> Mitigation: keep metadata descriptive, test with fake providers, and document that `providers_check`/live smoke remains the runtime readiness proof.
- [Risk] Clients may still stop tasks early if old guidance remains. -> Mitigation: update initialization instructions, prompts, resources, README, and `nextActions` together.

## Migration Plan

1. Add provider output-cadence metadata with conservative defaults.
2. Add progress metadata to public task summaries for running tasks.
3. Add `task_observe` and protocol tests for no-output, first-output, final-output, and timeout cases.
4. Update presentation `nextActions` to prefer observe/wait before stop.
5. Update guidance and README to describe provider-aware polling and silent Cursor behavior.
6. Keep existing lifecycle tools unchanged for compatibility.
