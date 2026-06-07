# Crate Source

**Generated:** 2026-06-07T12:00:00Z

Core of the MCP server. Modules split by concern; everything is `pub` because the crate is consumed as a library by its own binaries.

## STRUCTURE

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
├── task.rs                     # Lifecycle, spawn/observe/result (~110 KB)
├── task/
│   └── supervision.rs          # Child-process supervision
├── runtime.rs                  # Async runtime + panic/shutdown hooks
├── domain.rs                   # Shared domain types
├── guidance.rs                 # Next-action / report text (~26 KB)
├── claude_host.rs              # Host-runner socket mode
├── claude_interactive.rs       # Module facade
└── claude_interactive/         # (AGENTS.md)
```

## WHERE TO LOOK

| Task | Location |
|------|----------|
| Wire a new MCP tool | `tools.rs` (schema) → `server.rs` (dispatch) |
| Change task states/events | `task.rs` — expect large diffs |
| Adjust provider behavior | `provider.rs` |
| Tune diagnostic wording | `guidance.rs` |
| Modify protocol envelopes | `mcp.rs` |

## ANTI-PATTERNS

- Touch `task.rs` for a quick one-liner — the file is monolithic; refactor into smaller modules first.
- Bypass `runtime.rs` for shutdown handling — all signal and panic-path logic belongs there.
