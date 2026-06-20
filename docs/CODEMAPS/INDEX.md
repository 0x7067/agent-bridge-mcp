# Codemaps

**Last Updated:** 2026-06-20

## Architecture Overview

```mermaid
block-beta
    columns 3
    CLIENT["ACP or MCP Client<br/>(stdio)"]:1
    RT["Runtime<br/>runtime.rs"]:1
    ROUTER["ACP Router<br/>router_runtime.rs"]:1
    ADAPTER["MCP Adapter<br/>mcp_adapter.rs"]:1
    space:2
    TM["Task Manager<br/>task.rs"]:1
    PH["Provider Cmds<br/>provider.rs"]:1
    CH["Claude Host<br/>claude_host.rs"]:1
    space:2
    FS["State Store<br/>registry.json + fs"]:1

    CLIENT --> RT
    RT --> ROUTER
    RT --> ADAPTER
    ROUTER --> TM
    ADAPTER --> TM
    TM --> PH
    PH --> CH
    TM --> FS
    CH --> FS
```

## Module Heatmap

Approximated from commit frequency and structural centrality:

| Rank | Module Area | Approximate Commit Hits | Lives Today? |
|------|-------------|------------------------|--------------|
| 1 | `src/task.rs` + submodules | Very High | Yes |
| 2 | `src/router_runtime.rs` + `src/mcp_adapter.rs` | Very High | Yes |
| 3 | `src/provider.rs` | High | Yes |
| 4 | `src/claude_interactive/` | Medium-High | Yes |
| 5 | `src/server.rs` + diagnostics.rs | Medium | Yes |
| 6 | `src/runtime.rs` | Low | Yes |
| 7 | `src/mcp.rs` | Low | Yes |
| 8 | `src/domain.rs` | Low | Yes |
| 9 | `src/claude_host.rs` | Medium | Yes |

## Codemaps

| Area | File | Description |
|------|------|-------------|
| Backend | [backend.md](backend.md) | ACP router, MCP adapter, task lifecycle, runtime, and domain types |
| Integrations | [integrations.md](integrations.md) | Provider adapters, Claude host runner, and CLI bindings |
| State Store | [state-store.md](state-store.md) | Registry persistence, JSON state, and filesystem layout |
