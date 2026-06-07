---
status: accepted
date: 2025-03-??
---

# ADR-0001: Consolidate 14 Tools into 8 with Lean Responses

## Context

The initial MCP tool surface exposed fourteen discrete tools: `providers_list`, `providers_check`, `doctor`, `agent_preview`, `agent_spawn`, `agent_status`, `agent_observe`, `agent_wait`, `agent_transcript`, `agent_logs`, `agent_result`, `agent_list`, `agent_stop`, `agent_remove`. Early adoption revealed that the proliferation of narrowly-scoped tools increased cognitive load for LLM callers, caused duplicated response payloads (`nextActions`, `presentation`, `progress` copies), and complicated maintenance. The surface needed reduction without losing capability.

## Decision

We decided to fold six tools into four retained ones via parameterized options, reducing the public surface to eight tools:

- `agent_preview` → `agent_spawn` with `dryRun: true`
- `agent_status` → `agent_observe` with `limit: 0`
- `agent_wait` → `agent_observe` with `until: "final"`
- `agent_transcript` → `agent_observe` `events` (with `cursor`/`limit`)
- `agent_logs` → `agent_result` with `sections: ["stdout","stderr"]` and line pagination
- `providers_check` → `doctor` with `focus: "providers"`

Responses were simplified to a single `next` action list per tool, removing duplicate GUI presentation objects. Debug metadata moved behind `verbosity: "detailed"`. Raw evidence became opt-in via `agent_result` `sections`.

### Considered Alternatives

#### Keep all 14 tools

- Good, because explicit granularity is easier to search in documentation.
- Bad, because LLM callers struggle to pick the right tool; maintenance burden scales linearly.

#### Reduce to fewer than 8 tools

- Good, because even simpler surface.
- Bad, because combining fundamentally different verbs (e.g., observe vs. stop) into one tool harms MCP annotation semantics (`readOnlyHint`/`destructiveHint`) and makes debugging harder.

## Consequences

### Positive

- Reduced cognitive overhead for LLM tool-selection loops.
- Smaller JSON-RPC schema footprint; faster `tools/list` serialization.
- Unified response shape simplifies client-side parsing.

### Negative

- Backward compatibility breakage: old `taskId`-based registries are not migrated.
- Parameter-heavy tools require careful input validation (`deny_unknown_fields`).

### Neutral

- The `agent_` prefix replaced the mixed `task_`/`agent_` prefixes for consistency.

## Evidence

- **Commit(s):** `255a14a`, `7203b4b`
- **Key files changed:** `src/tools.rs`, `src/server.rs`, `src/task.rs`, `src/guidance.rs`, `tests/server_protocol.rs`
- **Blast radius:** 6 files, ~1600 lines changed.
- **Timeline:** Big-bang rewrite delivered in a single PR.
