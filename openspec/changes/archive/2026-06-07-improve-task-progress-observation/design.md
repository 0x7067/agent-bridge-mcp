## Context

Agent Bridge already stores lifecycle events, raw logs, normalized transcripts, presentation summaries, and `nextActions`. The weak point is the supervision loop used by human callers and harnesses: a running provider task can look idle because some providers, notably Cursor with JSON output, emit no stdout until completion. In the recent review-gate incident, Cursor was stopped after roughly 80 seconds even though its configured timeout was 600 seconds and recent successful Cursor reviews commonly completed after 50-190 seconds.

MCP protocol-level task notifications are not portable across the target hosts yet. The current server already supports request/response lifecycle tools, so the near-term design should improve observation through additive polling metadata and an optional long-poll request rather than relying on server-pushed notifications.

## Goals / Non-Goals

**Goals:**
- Help callers distinguish healthy silence from a true stalled task.
- Give harnesses provider-aware polling intervals, stop/fallback thresholds, and ready-to-call observation actions.
- Preserve raw `agent_logs`, bounded `agent_transcript`, and final `agent_result` as the source of evidence.
- Add an event-style observation path that can wait for new transcript/lifecycle activity or finalization without forcing clients to poll in a tight loop.
- Keep completion and verification boundaries unchanged.

**Non-Goals:**
- Do not implement MCP Tasks.
- Do not advertise `io.modelcontextprotocol/tasks`.
- Do not depend on server-pushed JSON-RPC notifications.
- Do not stream raw provider stdout/stderr unboundedly.
- Do not infer that a silent provider is correct or verified.

## Decisions

### Add `agent_observe` rather than overloading `agent_wait`

`agent_wait` answers one narrow question: "is this final yet?" A richer observation surface needs to answer "what changed since cursor N, what should I do next, and how long should I wait before escalating?" A dedicated `agent_observe` tool keeps those semantics explicit and aligns with the canonical `agent_*` surface.

`agent_observe` should accept:
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

Initial limits should be explicit: clamp `timeoutMs` to 120000ms and `limit` to 500 events. These limits are longer than `agent_wait`'s final-state wait cap because observe is a supervision call, but they remain bounded and request/response.

### Model output cadence as provider metadata

Provider adapters should expose output cadence guidance in `providers_list`, for example:

- Cursor: `cadence: "final_json"`, `recommendedSilentBudgetMs: 240000`, and `firstOutputExpected: "near_final"` because Cursor JSON mode commonly emits only at completion.
- Claude: `cadence: "provider_dependent"` with a conservative budget because output behavior differs between native Claude and host-runner paths.
- Kimi and Codex: `cadence: "provider_dependent"` unless adapter fixtures prove a stricter cadence.

This keeps harnesses from hardcoding Cursor-specific delays while still making the observed behavior discoverable.

The first metadata shape should be:

```json
{
  "outputCadence": {
    "cadence": "final_json | incremental | provider_dependent",
    "firstOutputExpected": "near_final | intermittent | unknown",
    "recommendedPollMs": 30000,
    "recommendedSilentBudgetMs": 240000,
    "fallbackAfterMs": 300000,
    "advisory": true,
    "note": "Cursor JSON output may be silent until final completion."
  }
}
```

`recommendedSilentBudgetMs` is the normal healthy-silence budget. `fallbackAfterMs` is when stop/fallback can become a reasonable next action if no output has appeared, capped by the configured task timeout when the timeout is shorter.

### Use adaptive next actions for running tasks

Running task `nextActions` should prefer `agent_observe` or `agent_wait` with provider-aware arguments. For silent final-output providers, the primary action should be a longer bounded observe/wait rather than `agent_stop`. Stop remains available but should be marked unsafe until the task is beyond the recommended observation budget or the caller has evidence that the task is no longer useful.

Progress computation should be deterministic:

- `elapsedMs`: `now - startedAt`, or `now - createdAt` before start.
- `lastEventAt`: timestamp of the latest transcript event, falling back to `updatedAt`.
- `lastOutputAt`: timestamp of the latest stdout/stderr/provider event, or `null`.
- `silentForMs`: `now - lastOutputAt` when output exists, otherwise `elapsedMs`.
- `effectiveSilentBudgetMs`: `min(recommendedSilentBudgetMs, timeoutSeconds * 1000)` when a timeout is configured.
- `secondsUntilRecommendedCheck`: seconds until the next `recommendedPollMs` interval, never negative.
- `stallRisk`: `none` for final tasks; `low` while elapsed/silence is below the silent budget; `medium` after the silent budget; `high` at or beyond `fallbackAfterMs` or within 30s of provider timeout.

Progress should be derived from persisted transcript/lifecycle state. The first implementation may derive from bounded transcript reads for correctness, but hot paths should avoid unbounded full-file rescans.

### Treat notifications as future compatibility

If a future host supports MCP notifications or task extensions, Agent Bridge can map the same observation events to that transport. This change should keep the internal event model and response shape compatible with a future stream, but the runtime API should remain request/response.

## Risks / Trade-offs

- [Risk] More metadata can look like certainty about provider health. -> Mitigation: use names like `stallRisk`, `expectedOutputCadence`, and `recommended`, and keep verification status separate.
- [Risk] Long-poll requests can tie up the task actor. -> Mitigation: implement observe without blocking the actor on child processes; use bounded waits and actor-friendly notifications or short sleep loops similar to `agent_wait`.
- [Risk] Provider cadence defaults may drift as CLIs change. -> Mitigation: keep metadata descriptive, test with fake providers, and document that `providers_check`/live smoke remains the runtime readiness proof.
- [Risk] Clients may still stop tasks early if old guidance remains. -> Mitigation: update initialization instructions, prompts, resources, README, and `nextActions` together.

## Migration Plan

1. Add provider output-cadence metadata with conservative defaults.
2. Add progress metadata to public task summaries for running and final tasks.
3. Add `agent_observe` and protocol tests for no-output, first-output, final-output, and timeout cases.
4. Update presentation actions and `nextActions` to prefer observe/wait before stop.
5. Update guidance and README to describe provider-aware polling and silent Cursor behavior.
6. Advertise one canonical `agent_*` lifecycle in `tools/list`; keep internal task records and identifiers as implementation details.
