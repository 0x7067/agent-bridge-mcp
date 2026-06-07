# Backend Codemap

**Last Updated:** 2026-06-07
**Entry Points:** `src/main.rs`, `src/runtime.rs`, `src/server.rs`, `src/task.rs`

## Architecture

```mermaid
block-beta
    columns 3
    MAIN["main.rs<br/>tokio::main"]:1
    RUNTIME["runtime.rs<br/>stdio loop + signals"]:1
    SERVER["server.rs<br/>JSON-RPC dispatch"]:1
    space:2
    TASK["task.rs<br/>manager façade"]:1
    DOMAIN["domain.rs<br/>types / enums"]:1
    GUIDANCE["guidance.rs<br/>prompts + resources"]:1

    MAIN --> RUNTIME
    RUNTIME --> SERVER
    SERVER --> TASK
    SERVER --> GUIDANCE
    TASK --> DOMAIN
```

## Key Modules

| Module | Purpose | Exports | Dependencies |
|--------|---------|---------|--------------|
| `main.rs` | Binary entrypoint | `main()` | `runtime::main_entry` |
| `runtime.rs` | Stdio loop, panic hook, shutdown signal handling, host-runner subcommand routing | `main_entry()` | `mcp`, `server`, `task`, `claude_host`, `libc` |
| `server.rs` | JSON-RPC method dispatcher (`initialize`, `tools/list`, `tools/call`, `prompts/*`, `resources/*`) | `handle_request()` | `tools`, `task`, `guidance`, `provider`, `diagnostics` |
| `server/diagnostics.rs` | `doctor` tool implementation + provider readiness/smoke checks | `doctor()` | `provider`, `env`, `tokio::process` |
| `task.rs` | Facade + `TaskManagerHandle` singleton + `TaskActor` message loop | `TaskManagerHandle`, `TaskRecord`, `Registry` | `task/*` submodules |
| `task/registry.rs` | Atomic registry load/save, legacy normalization, temp cleanup | `load_registry`, `save_registry`, `validate_registry_text` | `tokio::fs`, `serde_json` |
| `task/spawn.rs` | Argument validation, worktree creation, process launch, host-runner bridging | `validate_spawn_arguments`, `launch_task`, `safe_cwd` | `tokio::process`, `provider`, `domain` |
| `task/supervision.rs` | PID registry, process groups, signal termination, stdout/stderr draining | `register_active_pid`, `wait_for_child`, `drain_log` | `tokio::io`, `libc` |
| `task/complete.rs` | Completion classification, host-response ingestion, transcript scanning, git snapshots | `classify_completion`, `scan_partial_results`, `git_snapshot` | `std::fs`, `serde_json` |
| `task/review.rs` | Payload shaping, progress calculation, `next` action lists, listing/filtering | `public_task`, `observe_payload`, `review_packet`, `list_tasks` | `chrono`, `serde_json` |
| `domain.rs` | Core enums and strongly-typed wrappers | `ProviderKind`, `TaskMode`, `TaskStatus`, `FailureCategory`, `TimeoutSeconds`, `WorktreeName` | `serde` |
| `guidance.rs` | MCP prompts and resources; self-describing usage instructions | `INITIALIZATION_INSTRUCTIONS`, `prompt_definitions`, `resource_definitions` | `serde_json` |
| `mcp.rs` | Plain JSON-RPC 2.0 types | `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcId`, `JsonRpcError` | `serde` |
| `tools.rs` | Tool schemas, input deserializers, `deny_unknown_fields` | `ToolName`, `ToolCallParams`, `tool_definitions()` | `serde_json` |

## Data Flow

1. **Request arrival:** MCP client writes ND-JSON to stdin. `runtime.rs` (`BufReader` over stdin) deserializes into `JsonRpcRequest`.
2. **Dispatch:** `runtime.rs` calls `server::handle_request(request)`, which matches `method`.
3. **Tool call:** `tools/call` resolves `ToolName`, validates arguments, and fans out to task-manager methods or `doctor()`.
4. **Task lifecycle:** `TaskManagerHandle` serializes commands through an async MPSC channel to the `TaskActor`.
5. **Actor execution:** `TaskActor::run` processes `ActorCommand` variants (`Spawn`, `Stop`, `Get`, `List`, …) against in-memory registry and filesystem state.
6. **Spawn path:** `spawn.rs` validates → creates worktree (optional) → builds `ProviderCommand` → launches process (or host-runner task) → registers PID → begins IO drains.
7. **Supervision path:** `supervision.rs` `wait_for_child` polls exit status, timeout, and stderr denial. On completion, signals are escalated (SIGTERM → SIGKILL).
8. **Classification path:** `complete.rs` reads captured stdout/stderr, classifies exit status, builds `TaskCompletion`, and saves it to the registry.
9. **Observation path:** `review.rs` reads the registry and transcript JSONL, computing progress metrics and `next` action lists for the caller.
10. **Response:** `server.rs` wraps the result/error in `JsonRpcResponse` and writes ND-JSON to stdout.

## External Dependencies

| Crate | Purpose | Version |
|-------|---------|---------|
| `tokio` | Async runtime, process mgmt, IO, signals, channels | 1.52 |
| `serde` + `serde_json` | Serialization with `preserve_order` | 1.0 |
| `chrono` | Timestamps for registry/events | 0.4 |
| `uuid` | Unique temp filenames and agent IDs | 1.23 |
| `libc` | Signals, process groups, killpg | 0.2 |
| `pty-process` | Pseudo-terminal allocation for Claude | 0.5 |

## Related Areas

- [Integrations](integrations.md) — Provider command construction and Claude host runner wire protocol
- [State Store](state-store.md) — Registry persistence and filesystem layout
