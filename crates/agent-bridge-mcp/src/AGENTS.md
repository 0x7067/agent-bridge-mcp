# Crate Source

**Generated:** 2026-06-07T12:00:00Z

Core of the MCP server. Modules split by concern; everything is `pub` because the crate is consumed as a library by its own binaries.

## At a Glance

- `runtime.rs` owns the stdio loop, panic hook, and shutdown signals — never bypass it.
- `server.rs` dispatches all JSON-RPC methods; new tools start in `tools.rs` then wire here.
- `task.rs` is the lifecycle façade; real work lives in `task/{spawn,supervision,complete,review,registry}.rs`.
- `provider.rs` translates `(provider, mode)` into concrete CLI recipes and capabilities.
- `claude_interactive/` is the only provider with a PTY engine; keep PTY details contained there.

## Structure

```
src/
├── main.rs                   # Thin shim → runtime::main_entry()
├── bin/
│   └── agent-bridge-mcp-rs.rs # Alternate binary entrypoint
├── lib.rs                      # Public module re-exports
├── mcp.rs                      # JSON-RPC stdio framing
├── tools.rs                    # Eight-tool schemas + param structs
├── server.rs                   # Request router
├── server/
│   └── diagnostics.rs          # `doctor` tool internals
├── provider.rs                 # First-class provider registry
├── task.rs                     # Facade + TaskManagerHandle/TaskActor
├── task/
│   ├── spawn.rs                # Arg validation, worktree creation, process launch
│   ├── supervision.rs          # PID registry, signals, IO drainage
│   ├── complete.rs             # Exit classification, host-response ingest, git snapshots
│   ├── review.rs               # Payload shaping, progress, next-actions, listing
│   └── registry.rs             # Atomic registry load/save, legacy normalization
├── runtime.rs                  # Async runtime + panic/shutdown hooks
├── domain.rs                   # Shared domain types
├── guidance.rs                 # Next-action / report text (~26 KB)
├── claude_host.rs              # Host-runner socket mode
├── claude_interactive.rs       # Module facade
└── claude_interactive/         # (AGENTS.md)
```

## Where to Look

| Task | Location |
|------|----------|
| Wire a new MCP tool | `tools.rs` (schema) → `server.rs` (dispatch) |
| Change task states/events | `task.rs` façade + relevant `task/*.rs` submodule |
| Adjust provider behavior | `provider.rs` |
| Tune diagnostic wording | `guidance.rs` |
| Modify protocol envelopes | `mcp.rs` |
| Add a provider adapter | `provider.rs` (cmd builder) + `domain.rs` (enum) + `tests/` (fixture) |
| Fix crash-recovery / orphan worktrees | `task/registry.rs` (reconciliation) + `task/spawn.rs` (cleanup) |
| Improve observation UX | `task/review.rs` (progress, next-actions) |

## Anti-Patterns

- Touch `task.rs` for a quick one-liner — the file is a thin façade; refactor into the relevant `task/*.rs` submodule first.
- Bypass `runtime.rs` for shutdown handling — all signal and panic-path logic belongs there.
- Leak PTY details outside `claude_interactive/` — that's a provider-internal concern.
- Duplicate domain types across modules — `domain.rs` is the single source of truth.

## Going Deeper

- [Docs/agents architecture](../../../docs/agents/architecture.md) — module boundaries, spec-driven changes
- [Docs/agents guardrails](../../../docs/agents/guardrails.md) — stdout-as-protocol, no-new-tools rule
- [Docs/agents definition-of-done](../../../docs/agents/definition-of-done.md) — validation gates
- [Backend codemap](../../../CODEMAPS/backend.md) — entry points, data flow, external dependencies
- [Backend workflows](../../../WORKFLOWS/backend.md) — how to add a tool, add a provider, run gates
- [ADR/INDEX](../../../ADR/INDEX.md) — why the architecture is shaped this way

