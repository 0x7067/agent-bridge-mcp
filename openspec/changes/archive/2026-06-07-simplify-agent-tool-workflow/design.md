## Context

The public MCP lifecycle has already moved from legacy `task_*` tools to `agent_*` tools, and `simplify-agent-api-config` removed the mixed public vocabulary. The remaining complexity is workflow presentation: `tools/list`, initialization instructions, README, prompts, resources, and next-action metadata expose many valid lifecycle tools without clearly distinguishing the common path from diagnostic escape hatches.

The current implementation already has the primitives needed for a smaller workflow:

- `agent_observe` returns lifecycle state, transcript events, progress, and `nextActions`.
- `agent_result` returns final evidence and review-packet metadata.
- `doctor` aggregates setup diagnostics and can optionally run provider smoke checks.
- Lower-level tools such as `agent_logs`, `agent_transcript`, `agent_status`, and `agent_wait` remain useful when the primary path is insufficient.

## Goals / Non-Goals

**Goals:**

- Make the default Agent Bridge workflow feel like a small set of verbs.
- Preserve all current MCP tools, schemas, and response fields.
- Keep raw inspection and focused readiness tools available as diagnostic escape hatches.
- Align initialization instructions, prompts, resources, README, and tests around the same primary workflow.
- Ensure running-agent `nextActions` rank `agent_observe` first and final-agent `nextActions` rank `agent_result` first.

**Non-Goals:**

- Do not remove or rename any public MCP tool.
- Do not hide tools behind a capability flag or introduce an advanced-mode negotiation.
- Do not implement protocol-level MCP Tasks.
- Do not change provider adapter behavior, task registry shape, state migration, or cleanup safety.
- Do not make live provider smoke checks part of default CI or default caller workflow.

## Decisions

### Decision 1: Simplify guidance before shrinking the callable surface

The implementation will keep the existing 14-tool public MCP surface and simplify the instructions around it. The canonical path becomes:

1. `doctor` when setup, workspace, client, binary, or readiness is uncertain.
2. Optional `providers_check` when focused readiness or smoke verification is needed.
3. `agent_spawn` for the delegated task.
4. `agent_observe` for progress-aware polling.
5. `agent_result` for final evidence.
6. Caller-owned verification.
7. Intentional `agent_remove` after inspection when cleanup is desired.

Alternative considered: remove or merge tools immediately. That would reduce `tools/list` size but risks breaking harnesses and removes useful escape hatches before clients consistently follow `nextActions`.

### Decision 2: Mark advanced tools by description and guidance, not schema changes

`providers_check`, `agent_preview`, `agent_list`, `agent_status`, `agent_wait`, `agent_logs`, and `agent_transcript` will remain visible and callable. Their descriptions and guidance will frame them as focused readiness, launch inspection, presentation, finality, or diagnostic evidence tools.

Alternative considered: introduce `agent_inspect` as a single multiplexer. That would add a new abstraction and still require preserving the existing tools for compatibility, increasing rather than reducing the immediate surface.

### Decision 3: Let `nextActions` teach the path

For running agents, `agent_observe` stays the first recommended action because it returns progress, events, and action metadata. `agent_wait`, `agent_logs`, `agent_status`, and `agent_stop` remain subsequent choices. For final uninspected agents, `agent_result` stays first and cleanup remains gated for managed worktrees.

Alternative considered: remove secondary `nextActions`. Keeping them is useful for native clients and stalled-task recovery; the simplification is about ranking and wording.

## Risks / Trade-offs

- Guidance-only simplification may not reduce the raw `tools/list` count. Mitigation: make tool descriptions and initialization instructions explicitly distinguish primary and diagnostic tools.
- Over-deemphasizing raw logs/transcripts could make failure investigation harder. Mitigation: keep recovery and safety guidance explicit about when to use diagnostic tools.
- Tests may become too wording-sensitive. Mitigation: assert stable key phrases and ordering rather than full paragraphs.

## Migration Plan

1. Update OpenSpec deltas and tasks.
2. Update initialization instructions, prompts, resources, and README workflow.
3. Update tool descriptions for primary versus diagnostic lifecycle roles.
4. Add focused protocol/stdio tests for simplified guidance and stable tool surface.
5. Run formatting, focused tests, full cargo tests, OpenSpec validation, release build, installed-binary refresh, and installed-binary smoke.

Rollback is a normal git revert. Because no public API is removed, callers do not need a migration path.
