## Context

The current Rust bridge exposes a compact MCP surface for provider readiness and background task lifecycle management. Direct stdio tests cover initialization, tool listing, provider checks, command previews, lifecycle transitions, Claude diagnostics, and managed worktree cleanup.

The first live Codex MCP calls against the Rust server failed before provider logic ran because the host supplied a reserved `_meta` field next to `name` and `arguments` in `tools/call` params. The MCP specification reserves `_meta` for protocol metadata, so treating it like an unknown public tool field makes the implementation too strict for real MCP clients.

The bridge also needs an operational harness that reflects how it will actually be used: the main Codex thread owns judgment and integration, while provider tasks run bounded research, review, implementation, or command work and return inspectable results.

## Goals / Non-Goals

**Goals:**

- Make the Rust server compatible with real MCP hosts that send reserved request metadata.
- Preserve strict validation for user-facing tool arguments.
- Replace single-root workspace confinement with a path-list based `AGENT_BRIDGE_WORKSPACES` configuration.
- Add a production-binary compatibility fixture that covers the live Codex `_meta` shape.
- Define a repeatable workflow for provider checks, previews, task spawning, waiting, log inspection, result inspection, and cleanup.
- Keep live provider smoke checks opt-in and separate from deterministic CI tests.

**Non-Goals:**

- Do not loosen task argument schemas or allow arbitrary provider-specific fields.
- Do not preserve backwards compatibility for `AGENT_BRIDGE_ALLOWED_ROOT`.
- Do not make live Claude, Cursor, Kimi, or Codex execution mandatory in CI.
- Do not change public tool names, task lifecycle response shapes, or provider command policy.
- Do not add a new MCP transport or replace the local JSON-RPC stdio implementation.

## Decisions

### Decision 1: Accept `_meta` at the tool-call envelope only

`tools/call` params should tolerate `_meta` alongside `name` and `arguments`, then ignore it for dispatch. This should be implemented with an explicit `_meta` field on the tool-call envelope type while preserving unknown-field rejection for every other envelope field. That matches the MCP metadata model while preserving the bridge's local public API.

Alternative considered: remove `deny_unknown_fields` from all tool-related deserialization. That would fix `_meta`, but it would also weaken the public argument contract and hide client typos. The safer boundary is an explicit `_meta` exception plus strict tool argument parsing.

### Decision 2: Keep `arguments` strict and test the distinction

Unknown keys inside `arguments` should continue to return tool errors, as they do today for unsupported fields such as `maxTurns`. The regression suite should prove both sides: `_meta` on the envelope succeeds, unknown fields inside tool arguments fail.

Alternative considered: let the JSON Schema alone communicate `additionalProperties: false` and skip server-side validation. That would make behavior depend on each host respecting schemas before dispatch. The server should remain defensive.

### Decision 3: Add real-host compatibility coverage to the stdio harness

The compatibility test should run the production binary and send a `tools/call` request with params shaped like real Codex calls: `name`, `arguments`, and `_meta`. This catches client/server integration drift that unit-level request handlers and idealized fixtures miss.

Alternative considered: validate only through direct live Codex calls. Live calls are useful, but a deterministic production-binary fixture gives a small regression check that can run locally and in CI without provider credentials.

### Decision 4: Replace single-root confinement with `AGENT_BRIDGE_WORKSPACES`

Workspace confinement should use `AGENT_BRIDGE_WORKSPACES`, a platform path-list of allowed workspace roots. Each requested `cwd` is canonicalized and accepted only when it is equal to or inside one configured workspace. If the variable is unset, the server keeps the current-process directory as the only allowed workspace. The old `AGENT_BRIDGE_ALLOWED_ROOT` variable is removed rather than kept as a fallback.

Alternative considered: add `AGENT_BRIDGE_ALLOWED_ROOTS` while preserving `AGENT_BRIDGE_ALLOWED_ROOT`. That avoids breaking existing config, but the user explicitly prefers no backwards compatibility here. A single path-list variable also matches the real global Codex use case better than per-repo root edits.

### Decision 5: Treat live provider smoke checks as an intentional operator action

The runbook should keep deterministic fake-provider tests as the default proof. Live smoke should be a separate documented command or harness path that calls `providers_check(smoke: true)` and minimal read-only tasks only when the operator intentionally opts in.

Alternative considered: include live provider smokes in `cargo test`. That would catch local integration failures earlier, but it would make the default suite depend on installed CLIs, auth state, network behavior, and possible paid model usage.

### Decision 6: Use provider tasks as delegated evidence, not automatic truth

The main Codex thread should remain responsible for interpreting task results, checking diffs, running project gates, and deciding whether to integrate or discard provider output. Implementation tasks should default to managed worktree isolation in the documented workflow until a specific workflow chooses otherwise.

Alternative considered: treat provider success as equivalent to task completion. That would be too weak because providers can return plausible reports while tests fail, miss local constraints, or modify files outside the intended scope.

## Risks / Trade-offs

- `_meta` tolerance accidentally permits unvetted public inputs -> Limit tolerance to the MCP envelope and keep `arguments` validation strict.
- Workspace path-list parsing accepts unintended roots -> Canonicalize each workspace root, ignore empty entries, reject invalid configured paths with clear errors, and keep `cwd` canonicalization.
- Compatibility fixture diverges from actual Codex host behavior -> Include the observed `_meta` shape and keep a manual live smoke path for host-level checks.
- Live smoke harness spends quota or mutates a workspace -> Keep live smoke opt-in, use read-only prompts by default, and document worktree isolation for write-capable tasks.
- Operators leave managed worktrees and task records behind -> Make cleanup an explicit workflow requirement and keep `task_remove` after inspection in the runbook.
- Provider results are over-trusted -> Document that delegated output is evidence for the main thread, not a replacement for verification gates.

## Migration Plan

1. Add a failing production-binary fixture for `tools/call` with envelope `_meta`.
2. Update tool-call params parsing to tolerate `_meta` without changing public tool arguments.
3. Replace `AGENT_BRIDGE_ALLOWED_ROOT` with `AGENT_BRIDGE_WORKSPACES` in code, tests, and docs.
4. Add or update tests proving unknown fields inside `arguments` still fail.
5. Add the operational workflow and live-smoke instructions to docs.
6. Run `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `openspec validate support-real-world-mcp-workflow`.

Rollback: revert the parsing change and docs if the host compatibility behavior causes unexpected side effects. No persisted state migration is needed.

## Open Questions

- Should the runbook include provider preference defaults, such as Kimi for second-opinion review and Claude/Codex for implementation, or keep provider selection purely capability-based in v1?
