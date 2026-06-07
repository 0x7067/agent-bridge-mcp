## Context

Agent Bridge recently moved the public lifecycle from `task_*` tools to `agent_*` tools, but the payloads still expose `taskId`, next actions still pass `taskId`, and some guidance/spec text still names old `task_*` workflows. That leaves callers with two vocabularies for the same thing.

Runtime task state defaults to `~/.agent-bridge-mcp/state`, while `doctor` falls back to `~/.agent-bridge`. A caller who omits `AGENT_BRIDGE_STATE_DIR` can therefore get misleading diagnostics about a different state location than the task manager actually uses.

## Goals / Non-Goals

**Goals:**

- Make the advertised MCP lifecycle purely agent-oriented: `agent_*` tools with `agentId` arguments and response fields.
- Reject old `taskId` public arguments instead of maintaining compatibility aliases.
- Generate new public identifiers with an `agent_` prefix.
- Keep implementation changes bounded to MCP API/config simplification, docs, specs, and tests.
- Use one state directory default everywhere: `~/.agent-bridge-mcp/state`.

**Non-Goals:**

- Do not implement protocol-level MCP Tasks.
- Do not add a second compatibility tool family or hidden fallback aliases.
- Do not refactor every internal Rust variable or type named `Task`; internal implementation terminology can remain where it does not leak into public MCP contracts.
- Do not migrate existing on-disk registries; this is an intentional breaking change.

## Decisions

### Decision 1: Break the public identifier field

The public `agent_*` lifecycle accepts and returns `agentId` only. `taskId` is rejected by the existing unknown-field validation path.

Alternative considered: accept both `agentId` and `taskId` while documenting `agentId` as preferred. That keeps ambiguity in the API and was explicitly rejected.

### Decision 2: Keep the internal task manager shape

The Rust actor can still use internal `TaskRecord`, `task_id`, and `tasks` naming while rendering public MCP payloads as agents. This avoids a broad mechanical refactor that would increase risk without making the public surface simpler.

Alternative considered: rename every internal type and field to agent terminology. That would touch most of `task.rs`, persisted paths, and actor messages while adding little external clarity.

### Decision 3: Change generated IDs and registry serialization to agent terminology

New lifecycle records use `agent_` plus UUID-hex identifiers. This makes copied identifiers self-describing and removes the last visible `task_` prefix from normal public flows.

Alternative considered: keep `task_...` values under the new `agentId` field. That preserves internal provenance but leaks old vocabulary into the simplified public surface.

Existing registries serialized with the old public `taskId` field are treated as incompatible and fail startup with the existing clear registry parse/startup error path. The change does not attempt to rewrite old IDs or mixed registries because the user explicitly rejected backward compatibility.

### Decision 4: Use the runtime state default everywhere

`doctor` adopts the task manager default `~/.agent-bridge-mcp/state`. `AGENT_BRIDGE_STATE_DIR` remains the only override for MCP server state.

Alternative considered: keep `doctor`'s older `~/.agent-bridge` fallback as a diagnostic legacy path. That makes minimal config ambiguous and is not worth preserving.

## Risks / Trade-offs

- Existing callers that send `taskId` will fail with unknown-argument errors -> Update README, guidance, specs, and tests with the exact migration.
- Existing registries using `taskId` fields do not load after the break -> This is accepted as part of the no-compatibility constraint; use a fresh state dir or discard old task records.
- Some internal errors and file paths may still say "task" -> Keep internal implementation naming where it is not part of advertised MCP input/output; revisit only if it leaks into public docs or schemas.
- OpenSpec baselines contain older `task_*` wording from archived transitions -> This change updates the modified capabilities it touches, and implementation tests guard the live MCP surface.

## Migration Plan

1. Update the OpenSpec deltas and reviewed implementation plan.
2. Change schemas, argument parsing, response rendering, next actions, and generated IDs.
3. Align `doctor` state default with task runtime state default.
4. Update README, guidance, and tests.
5. Run formatting, focused tests, full cargo tests, OpenSpec validation, release build, installed-binary comparison, and installed-binary smoke.

Rollback is a normal git revert before release. After release, callers must migrate from `taskId` to `agentId`; there is no compatibility path in this change.
