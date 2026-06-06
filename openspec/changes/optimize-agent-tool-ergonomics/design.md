# Design

## Goals

1. Cut per-call (especially per-poll) token cost by removing duplication and GUI chrome.
2. Shrink and de-overlap the public tool surface for discoverability (14 → 8).
3. Make the API code-execution / Tool-Search friendly: pagination-first, evidence on
   demand, annotated, deterministic.

## Constraint that unlocks the design

Only LLM agents consume these payloads. The `presentation` object (display title,
subtitle, status tone, timestamps, and a ten-entry UI `actions` array with availability
state/reason) exists to let a native client render chips and buttons. With no such client,
it is pure overhead and is removed rather than gated behind a flag.

## Layer A — Lean, deduplicated agent-facing responses (default)

### Current duplication (problem)

`observe_payload` returns, in one response:

```
{ agentId, status, isFinal,
  agent:        public_task(...),   // includes progress, presentation, nextActions
  presentation: agent.presentation, // duplicate (contains actions[] AND nextActions)
  progress:     agent.progress,     // duplicate
  events, nextCursor, timedOut,
  nextActions:  agent.nextActions } // duplicate
```

Net effect: `nextActions` ×4, `progress` ×3, `presentation` ×2 per poll.

### Target envelope

One canonical lean envelope, every field once:

```
agent_observe -> {
  agentId, status, isFinal, phase,
  progress,            // once
  events, nextCursor,  // the incremental transcript
  timedOut,
  next                 // single deduplicated next-step list
}
```

- Remove the `agent` full-record echo and both `presentation` copies from agent-facing
  responses. Drop the GUI `actions` array entirely.
- `next` is a single array of `{ tool, arguments, safety, reason }` with a short `reason`
  (no prose duplicated across four locations). It keeps the existing machine-actionable
  shape (ready-to-call arguments + safety classification) that callers already rely on.
- `verbosity: "detailed"` (opt-in, default `compact`) re-adds debug metadata
  (timestamps, `profile`, `promptStrategy`, diagnostics) for the rare debugging case.

`agent_list` summaries follow the same rule: lean per-agent records (identity, status,
phase, progress, `next` primary action) without the GUI presentation/action blob.

## Layer B — Consolidate 14 → 8 via subsuming parameters

| Keep (8) | Subsumes (removed) | Mechanism |
|---|---|---|
| `agent_spawn` | `agent_preview` | `dryRun: true` returns the launch preview (command, cwd, env, profile, isolation) without spawning |
| `agent_observe` | `agent_status`, `agent_wait`, `agent_transcript` | `until: "now"` (default; return state + new events immediately) or `until: "final"` (block to finality or `timeoutMs`, replacing wait); `limit: 0` = state-only (replacing status); `events` already are the transcript |
| `agent_result` | `agent_logs` | `sections: ["summary","stdout","stderr","transcript","diff","changedFiles"]` (default `["summary","changedFiles"]`) with `maxBytes`, `stdoutLine`, `stderrLine`, `cursor` pagination |
| `agent_list` | — | unchanged surface, lean records |
| `agent_stop` | — | unchanged |
| `agent_remove` | — | unchanged |
| `doctor` | `providers_check` | `focus: "providers" \| "all"` (default `all`) runs only the readiness section; `smoke`, `providers`, `aggregateTimeoutMs`, `providerTimeoutMs` carry over unchanged |
| `providers_list` | — | kept: cheap, side-effect-free static capability lookup, distinct purpose |

`doctor` already runs "the same bounded provider smoke readiness behavior as
`providers_check`" with "the same validation and deduplication semantics," so the fold is a
re-pointing of the public entry point, not a behavior change. The readiness engine
(filtering, aggregate budget, provider-specific budgets, smoke phases, stderr diagnostics)
is unchanged; only its advertised tool name moves to `doctor`.

## Layer C — Code-execution / Tool-Search friendliness

- **Evidence on demand.** Default `agent_result` returns `summary` + `changedFiles` only.
  Full `stdout`/`stderr`/`diff`/`transcript` are fetched explicitly via `sections` and
  paginated, so large intermediate data stays out of the model context (the core
  code-execution-with-MCP principle).
- **Tool annotations.** Each tool gets MCP `annotations`: `readOnlyHint: true` for
  `doctor`, `providers_list`, `agent_observe`, `agent_result`, `agent_list`;
  `destructiveHint: true` for `agent_remove` (and `agent_stop`); `idempotentHint: true`
  where applicable. Tool-Search-capable clients can then load `agent_spawn`/`agent_observe`/
  `agent_result` first and defer the diagnostic/control tools.
- **Guidance.** A new `agent-bridge://guidance/code-execution` resource documents:
  poll compactly with `agent_observe` (`until`/`timeoutMs`), fetch evidence sections on
  demand from `agent_result`, keep raw logs/diffs out of context until needed, and run
  caller-owned verification. Initialization instructions and existing prompts/resources are
  updated to name the eight-tool surface.

## Migration

Intentional breaking change, consistent with the repo's prior `agent_*`/`agentId`
simplification. Persisted `agentId`/`taskId` values are unchanged.

| Removed tool | Replacement |
|---|---|
| `agent_preview` | `agent_spawn` with `dryRun: true` |
| `agent_status` | `agent_observe` with `limit: 0` |
| `agent_wait` | `agent_observe` with `until: "final"`, `timeoutMs` |
| `agent_transcript` | `agent_observe` `events` (with `cursor`/`limit`) |
| `agent_logs` | `agent_result` with `sections: ["stdout","stderr"]` + line pagination |
| `providers_check` | `doctor` with `focus: "providers"` (+ `smoke`/filters as before) |

Response-shape break: callers that read `result.agent`, `result.presentation`, or the
duplicated `nextActions`/`progress` copies switch to the single top-level fields and `next`.

## Removal strategy

Tools are removed from `tools/list` and the `ToolName` enum once the subsuming parameters
are covered by protocol and stdio tests. No hidden aliases are advertised; if compatibility
shims are ever needed they are feature-gated, not part of the canonical surface.
