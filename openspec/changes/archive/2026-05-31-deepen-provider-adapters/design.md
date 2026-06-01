## Context

One runtime module previously owned the whole MCP server: tool definitions, argument allowlists, provider capability metadata, task validation, provider command construction, environment allowlists, provider availability checks, task lifecycle management, git snapshots, log handling, and stdio transport.

The provider concern is the best first maintainability boundary because it is both cohesive and repeatedly touched. A provider change currently crosses these areas:

```text
provider metadata
      |
      v
tool schemas -> spawn validation -> command builder -> env builder -> providers_check
      \_______________________________________________________________/
                          same provider concept
```

The refactor should preserve public behavior while making that provider concept explicit.

## Goals / Non-Goals

**Goals:**
- Create a small provider adapter interface that hides provider-specific decisions behind provider-neutral calls.
- Keep `TaskManager` focused on task lifecycle: persistence, process launch, logs, status, results, worktrees, and git snapshots.
- Keep public MCP tool names, argument names, defaults, returned shapes, and safety behavior unchanged.
- Preserve the existing low-dependency posture.
- Make provider behavior easier to test directly without exercising the whole MCP request path.

**Non-Goals:**
- No new providers.
- No redesign of task lifecycle state or registry persistence.
- No change to provider CLI flags except to preserve or clarify existing behavior through tests.
- No TypeScript migration or new build step.
- No public plugin system for third-party provider adapters.

## Decisions

1. **Use internal provider adapter modules, not classes.**

   Each adapter should be a plain object or function collection with a common shape. This fits the current ESM/stdlib codebase and avoids introducing inheritance for four concrete providers.

   Example shape:

   ```js
   const codexAdapter = {
     name: "codex",
     modes: ["research", "review", "implement", "command"],
     options: { thinking: ["low", "medium", "high", "xhigh"] },
     command(task, context) { /* returns command descriptor */ },
     env() { /* returns provider env */ }
   };
   ```

   Alternative considered: keep one `buildTaskCommand()` switch and only extract helper functions. That lowers diff size but leaves capability metadata, env policy, smoke commands, and option validation scattered.

2. **Put cross-provider validation in a registry facade.**

   Generic spawn rules still belong outside individual adapters: known provider names, required prompt, prompt byte cap, cwd safety, timeout clamping, isolation, and worktree name rules. Provider-specific option checks should ask the selected adapter whether `mode`, `effort`, or `thinking` is valid.

   This keeps security-sensitive task validation centralized while preventing the validator from knowing every provider's option matrix.

3. **Keep command descriptors unchanged.**

   `buildTaskCommand()` can remain as an exported compatibility boundary for tests and current callers, but internally it should delegate to the provider registry. The returned descriptor shape should stay `{ command, args, cwd, timeoutSeconds, task }`.

4. **Make provider checks use the same adapter path as task spawn.**

   `providers_check` currently has separate command-resolution logic from task command construction. The refactor should let each adapter expose its version command and smoke command, so check behavior cannot drift from spawn behavior.

5. **Test through stable boundaries first, internals second.**

   Existing high-level tests should continue to cover MCP tool behavior. New focused tests should cover adapter capabilities, command descriptors, env policy, and smoke command construction. Tests should not assert file layout; they should assert behavior.

## Risks / Trade-offs

- **Risk: accidental CLI flag drift** -> Mitigation: move tests before extraction where practical, then assert exact current command slices for Claude, Cursor, Kimi, and Codex.
- **Risk: over-fragmenting a small repo** -> Mitigation: extract only provider-owned code first; leave task lifecycle and protocol routing in place.
- **Risk: hidden coupling between validation and tool schemas** -> Mitigation: derive provider enums and option metadata from the registry where practical, or add a focused test that `providers_list` and `tools/list` expose compatible metadata.
- **Risk: environment policy regression** -> Mitigation: keep provider env tests, especially Claude's removal of injected `ANTHROPIC_BASE_URL`.
- **Risk: smoke checks diverge from runtime command behavior** -> Mitigation: adapter owns both `versionCommand()` and `smokeCommand()`.

## Migration Plan

1. Add tests that pin current provider adapter behavior before extracting it.
2. Extract provider registry and adapter modules behind the existing exported functions.
3. Update `TaskManager` to call the registry instead of provider-specific switch logic.
4. Keep README behavior unchanged; add a short maintainer note only if useful.
5. Run the project test suite and a lightweight MCP smoke check.

Rollback is straightforward because this is internal restructuring: revert the extraction commit and keep existing public tests as the safety net.

## Open Questions

- None for the first pass. Task lifecycle extraction remains a follow-up candidate after provider adapters are stable.
