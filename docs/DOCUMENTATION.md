# Project Overview

**Last updated:** 2026-06-07

Agent Bridge MCP is a Rust stdio MCP server that delegates bounded tasks from a primary coding agent to local provider agents (Claude Code, Codex, Cursor, Kimi/Pi, Antigravity). It exposes a unified lifecycle — preview, spawn, observe, inspect, stop, remove — while keeping the caller responsible for verification.

## Tech Stack

| Package | Version | Purpose |
|---------|---------|---------|
| Rust | 2024 edition | Language and compiler |
| Cargo | Resolver 3 | Build and dependency management |
| tokio | 1.52 | Async runtime, process management, signals, IO |
| serde + serde_json | 1.0 | Serialization with deterministic field ordering |
| chrono | 0.4 | RFC3339 timestamps |
| uuid | 1.23 | V4 identifiers for temp files and agent IDs |
| libc | 0.2 | UNIX signals and process groups |
| pty-process | 0.5 | Pseudo-terminal allocation for Claude interactive mode |

## Core Features

### Delegation Surface

Eight MCP tools constitute the public API:

| Tool | Former Names | Capability |
|------|-------------|------------|
| `providers_list` | — | Enumerate providers, modes, profiles, cadences |
| `doctor` | `providers_check` | Diagnostics: setup, readiness, smoke tests |
| `agent_spawn` | `agent_preview` | Queue a task (optionally dry-run) |
| `agent_observe` | `agent_status`, `agent_wait`, `agent_transcript` | Stream events, block to finality, or poll state |
| `agent_result` | `agent_logs` | Review packet + on-demand evidence sections |
| `agent_list` | — | Query and filter active/recent tasks |
| `agent_stop` | — | Terminate a running task |
| `agent_remove` | — | Purge a finished task and its worktree |

Links: [Backend Codemap](CODEMAPS/backend.md), [Backend Workflows](WORKFLOWS/backend.md), [ADR-0001](ADR/0001-consolidate-eight-tools.md)

### Task Lifecycle

Tasks progress through a strictly-validated state machine:

`Queued → Running → Succeeded|Failed|Stopped|FailedStale → Removed`

On server startup, orphaned `Queued`/`Running` records are reconciled to `FailedStale` and their worktrees reclaimed.

Links: [Data Model](DATA-MODEL.md), [Backend Codemap](CODEMAPS/backend.md)

### Provider Adapters

Five first-class providers are supported, each with a typed adapter defining command construction, environment filtering, denial detection, and output cadence:

- **Claude** — Interactive via owned PTY host runner (Unix socket)
- **Codex** — Noninteractive fork/exec; detects sandbox denials
- **Cursor** — Noninteractive fork/exec; JSON-final output cadence
- **Kimi** — Noninteractive fork/exec; supports thinking levels
- **Antigravity** — Noninteractive fork/exec; research/review modes with `--sandbox`

Links: [Integrations](INTEGRATIONS.md), [Integrations Codemap](CODEMAPS/integrations.md), [ADR-0002](ADR/0002-harden-task-lifecycle.md), [ADR-0005](ADR/0005-claude-pty-host-runner.md)

### Worktree Isolation

Mutable tasks (`implement`, `command`) may request `isolation: Worktree`. Agent Bridge creates a disposable git worktree on a branch named `agent-bridge/...`, runs the provider inside it, and preserves the worktree until explicit `agent_remove`. This isolates provider mutations from the caller's original working directory.

Links: [Data Model](DATA-MODEL.md), [Business Context](BUSINESS-CONTEXT.md)

### Observability & Diagnostics

- **Transcripts:** Every stdout/stderr line and lifecycle event is appended to `transcript.jsonl` with redaction.
- **Progress:** `agent_observe` returns a computed `progress` object with `stallRisk`, `elapsedMs`, `timeoutRemainingMs`, and `recommendedNextTool`.
- **Diagnostics:** On failure, a structured `diagnostic` JSON includes failure category, excerpted/redacted stdout/stderr, and provider metadata.
- **Doctor:** Built-in readiness checks for workspace, state directory, client config, binary freshness, and per-provider smokability.

Links: [Data Model](DATA-MODEL.md), [Security Patterns](SECURITY.md)

### Quality Gates

Four hard CI gates enforce cleanliness:

1. `cargo fmt --all --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo machete`
4. `jscpd` < 5%

Two informational reports accompany every build: complexity hotspots and module dependency graphs.

Links: [ADR-0004](ADR/0004-static-intelligence-gates.md), [Setup](SETUP.md)

## Documentation Roadmap

**New to the project?**
1. [Getting Started](GETTING-STARTED.md) — clone, build, run tests
2. [System Context](ARCHITECTURE/system-context.md) — who uses this and why
3. [Container Architecture](ARCHITECTURE/containers.md) — what's inside the black box

**Want to understand design decisions?**
1. [ADR/INDEX.md](ADR/INDEX.md) — all architectural decisions
2. [Data Model](DATA-MODEL.md) — how state is shaped and persists
3. [Business Context](BUSINESS-CONTEXT.md) — domain rules and workflows

**Ready to modify code?**
1. [Backend Codemap](CODEMAPS/backend.md) — navigate the server and task machinery
2. [Integrations Codemap](CODEMAPS/integrations.md) — understand provider wiring
3. [Backend Workflows](WORKFLOWS/backend.md) — patterns for extending tools/providers
4. [Testing Workflows](WORKFLOWS/unit-tests.md) — write deterministic fake-provider tests
