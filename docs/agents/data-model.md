# Data Model

Two persistent concepts: the task registry and per-task evidence directories.

## At a Glance

- `registry.json` — atomic JSON catalog of all tasks (write-to-temp + rename).
- `tasks/<agentId>/` — per-task directory: `stdout.log`, `stderr.log`, `transcript.jsonl`.
- `worktrees/` — disposable git worktrees for isolated tasks.

## Task States

```
Queued → Running → Succeeded | Failed | Stopped | FailedStale → Removed
```

Illegal transitions are rejected. On server startup, orphaned `Queued`/`Running` records become `FailedStale` and their worktrees are reclaimed.

## Key Entities

### TaskRecord

| Concept | Code | Meaning |
|---------|------|---------|
| `agentId` | `UUID` string | Canonical identifier (legacy `taskId` normalized on load) |
| `provider` | `ProviderKind` | Which CLI ran the work (`claude`, `cursor`, `kimi`, `codex`, `forge`, `antigravity`) |
| `mode` | `TaskMode` | `research`, `review`, `implement`, `command` |
| `status` | `TaskStatus` | `Queued`, `Running`, `Succeeded`, `Failed`, `Stopped`, `FailedStale`, `Removed` |
| `isolation` | `Isolation` | `None` or `Worktree` |
| `worktreeManaged` | bool | Did Agent Bridge create and own the worktree? |
| `agentDir` | path string | Private directory for captured logs and transcript |
| `resultInspectedAt` | timestamp | When caller first viewed `agent_result` |
| `transcriptAvailable` | bool | Does `transcript.jsonl` exist with readable events? |
| `finalResultDetected` | bool | Was a conclusive `provider_result` event recorded? |
| `diagnostic` | JSON | Structured failure snapshot (redacted excerpts, category, metadata) |

### Registry

- Simple `BTreeMap<agentId, TaskRecord>` wrapped in JSON.
- Atomically persisted via temp file + rename to prevent corruption.
- Legacy field normalization (`taskId`→`agentId`, `taskDir`→`agentDir`) on deserialization.

## Filesystem Layout

```
~/.agent-bridge-mcp/state/
├── registry.json
├── tasks/
│   └── <agentId>/
│       ├── stdout.log
│       ├── stderr.log
│       └── transcript.jsonl
└── worktrees/
    └── <provider>-<mode>-<suffix>/
```

## Going Deeper

- [Full data model](../DATA-MODEL.md) — entity relationship diagram, lifecycle state machine, schema conventions
- [State store codemap](../CODEMAPS/state-store.md) — registry load/save implementation details
- [Backend codemap](../CODEMAPS/backend.md) — how `TaskActor` interacts with the registry
