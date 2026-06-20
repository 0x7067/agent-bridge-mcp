# State Store Codemap

**Last Updated:** 2026-06-07
**Entry Points:** `task/registry.rs`, `task.rs` (`TaskActor`), filesystem layout

## Architecture

```mermaid
block-beta
    columns 3
    REG["registry.json<br/>Atomic master catalog"]:1
    TASKDIR["tasks/&lt;agentId&gt;/<br/>Per-task directory"]:1
    WT["worktrees/<br/>Disposable git branches"]:1

    REG --> TASKDIR
    TASKDIR --> WT
```

## Key Modules

| Module | Purpose | Exports | Dependencies |
|--------|---------|---------|--------------|
| `task/registry.rs` | Atomic JSON serialization/deserialization, temp cleanup, legacy field normalization, home-dir expansion | `load_registry`, `save_registry`, `validate_registry_text`, `normalize_legacy_registry_fields_exported`, `expand_home` | `tokio::fs`, `serde_json`, `uuid` |
| `task.rs` | Singleton `MANAGER` (`OnceCell`), `TaskActor` mailbox loop, startup reconciliation | `TaskManagerHandle`, `TaskActor`, `max_active_tasks()` | `tokio::sync`, `tokio::fs` |

## Data Flow

1. **Initialization:** `TaskManagerHandle::from_env()` expands `AGENT_BRIDGE_STATE_DIR` (default `~/.agent-bridge-mcp/state`), ensures `tasks/` subdirectory exists, loads `registry.json`.
2. **Crash reconciliation:** On load, any `Queued` or `Running` records are transitioned to `FailedStale` and their managed worktrees are forcibly removed via `git worktree remove -f`.
3. **Mutation:** Every actor command that modifies registry state triggers `save_registry()` asynchronously.
4. **Atomic writes:** `save_registry` writes to a uniquely-named temp file (`registry.json.tmp-{pid}-{uuid}`), then performs an atomic `fs::rename` to `registry.json`. Old temps are garbage-collected on subsequent loads.
5. **Per-task files:** During spawn, an `agent_dir` is created at `${STATE_DIR}/tasks/${agentId}/`. It houses:
   - `stdout.log` — capped stdout capture (max 1 MiB)
   - `stderr.log` — capped stderr capture (max 1 MiB)
   - `transcript.jsonl` — append-only ND-JSON event stream
6. **Worktrees:** When `isolation: Worktree` is requested, `create_worktree()` places a new git worktree under `${STATE_DIR}/worktrees/` linked to the caller's repo.

## Filesystem Layout

```
~/.agent-bridge-mcp/state/
├── registry.json                ← Master catalog (atomic replace)
├── registry.json.tmp-*           ← Ephemeral write buffers (cleaned on load)
├── tasks/
│   └── <agentId>/
│       ├── stdout.log
│       ├── stderr.log
│       └── transcript.jsonl
└── worktrees/
    └── <provider>-<mode>-<suffix>/   ← Git worktrees (removed during managed cleanup)
```

## External Dependencies

| Crate | Purpose | Version |
|-------|---------|---------|
| `tokio::fs` | Async file operations, atomic writes | bundled in tokio |
| `serde_json` | Pretty-printed JSON serialization with preserve_order | 1.0 |
| `uuid` | Random suffixes for temp filenames | 1.23 |

## Related Areas

- [Backend](backend.md) — TaskActor lifecycle and registry consumers
- [Integrations](integrations.md) — Git worktree mechanics
