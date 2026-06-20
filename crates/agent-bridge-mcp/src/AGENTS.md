# Crate Source

**Generated:** 2026-06-07T12:00:00Z

Core of the agent-bridge runtime. Modules split by concern; public modules are
consumed as a library by the crate's own binaries.

## At a Glance

- `runtime.rs` owns CLI parsing, stdio dispatch, panic hook, and shutdown signals — never bypass it.
- `router_runtime.rs` runs the default ACP router stdio loop and one routed prompt turn.
- `mcp_adapter.rs` is the minimal MCP compatibility surface with `agent_delegate` and `agent_evidence`.
- `server.rs` is internal diagnostics only; new public protocol work belongs in `router_runtime.rs` or `mcp_adapter.rs`.
- `task.rs` is the lifecycle façade; real work lives in `task/{input,spawn,supervision,complete,review,registry}.rs`.
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
├── mcp_adapter.rs              # Minimal MCP adapter (agent_delegate, agent_evidence)
├── router_runtime.rs           # ACP router stdio loop + routed turn execution
├── router.rs                   # Router policy, attempt input, terminal classification
├── server.rs                   # Internal diagnostics
├── server/
│   └── diagnostics.rs          # Provider readiness diagnostics
├── provider.rs                 # First-class provider registry
├── task.rs                     # Facade + TaskManagerHandle/TaskActor
├── task/
│   ├── input.rs                # Task spawn input structs
│   ├── spawn.rs                # Arg validation, worktree creation, process launch
│   ├── supervision.rs          # PID registry, signals, IO drainage
│   ├── complete.rs             # Exit classification, host-response ingest, git snapshots
│   ├── review.rs               # Payload shaping, progress, next-actions, listing
│   └── registry.rs             # Atomic registry load/save, legacy normalization
├── runtime.rs                  # Async runtime + panic/shutdown hooks
├── domain.rs                   # Shared domain types
├── claude_host.rs              # Host-runner socket mode
├── claude_interactive.rs       # Module facade
└── claude_interactive/         # (AGENTS.md)
```

## Where to Look

| Task | Location |
|------|----------|
| Wire a new ACP method | `router_runtime.rs` |
| Change adapter tools | `mcp_adapter.rs` |
| Change task states/events | `task.rs` façade + relevant `task/*.rs` submodule |
| Adjust provider behavior | `provider.rs` |
| Tune diagnostic wording | `server/diagnostics.rs` |
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
