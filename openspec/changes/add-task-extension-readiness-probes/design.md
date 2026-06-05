## Context

Agent Bridge already has a stable provider task lifecycle through `task_*` tools. The `explore-mcp-task-support` compatibility memo blocks protocol-level MCP Tasks implementation because the legacy 2025-11-25 task surface and the newer `io.modelcontextprotocol/tasks` extension have different negotiation and method shapes.

The useful next step is not task execution. It is gathering deterministic readiness evidence about what task-extension capability metadata clients can send over stdio and how Agent Bridge should report that evidence without advertising task support.

## Goals / Non-Goals

**Goals:**

- Detect task-extension capability metadata in MCP client/request metadata without enabling protocol task execution.
- Classify observed client support as unavailable, legacy-only, extension-capable, unsupported, or unknown.
- Provide a diagnostic surface that can be tested through stdio fixtures and used by future task-support work.
- Keep the existing Agent Bridge `task_*` lifecycle as the only execution path.

**Non-Goals:**

- Do not advertise `io.modelcontextprotocol/tasks` server support.
- Do not implement `tasks/get`, `tasks/update`, `tasks/cancel`, `tasks/result`, or `tasks/list`.
- Do not return `CreateTaskResult`.
- Do not change task registry ownership, task IDs, cancellation semantics, or cleanup behavior.
- Do not infer support from a host name alone.

## Decisions

### Decision: Probe passively from metadata

Task-extension readiness should inspect MCP request metadata and deterministic fixtures rather than actively changing server capabilities.

Rationale: Current host support is uncertain and task support requires explicit negotiation. Passive probing gives evidence without misleading clients.

Alternatives considered:

- Advertise server task support and see whether clients use it. Rejected because it would claim unsupported behavior.
- Add protocol task methods that always return method-not-found. Rejected because the existing MCP method-not-found behavior already covers unsupported methods.

### Decision: Expose diagnostic classification, not execution

The readiness surface should return a small structured classification with observed extension identifiers, legacy indicators, recommended next step, and a boolean `serverAdvertisesTasks: false`.

Rationale: Future implementation decisions need normalized evidence, but clients must not confuse diagnostics with support.

Alternatives considered:

- Put classification only in prose guidance. Rejected because fixtures and future code need stable fields.
- Persist all observed client metadata. Rejected for privacy and unnecessary state growth; readiness can be request-scoped.

### Decision: Include legacy and extension fixtures

Tests should cover:

- no task metadata
- legacy 2025-11-25-style task metadata or capabilities
- current `io.modelcontextprotocol/tasks` extension metadata
- unknown task-like metadata

Rationale: The blocked compatibility memo specifically identified migration differences, so fixtures need to prevent accidental conflation.

## Risks / Trade-offs

- Hosts may not expose per-request capabilities in the same place -> Keep parser tolerant and classify unknown shapes without failing ordinary requests.
- Diagnostic fields may be mistaken for support -> Include `serverAdvertisesTasks: false` and guidance that task execution still uses `task_*`.
- Future extension names may change -> Keep unknown extension identifiers visible in diagnostics rather than hard-failing.

## Migration Plan

1. Add the readiness classifier and fixtures.
2. Expose diagnostics through a narrow diagnostic surface or existing setup/guidance output.
3. Keep all protocol task support disabled.
4. Use collected fixture evidence to decide a later implementation change.
