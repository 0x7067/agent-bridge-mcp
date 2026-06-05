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

The server should inspect:

- `initialize.params.capabilities.experimental["io.modelcontextprotocol/tasks"]`
- `initialize.params.capabilities.experimental["tasks"]`
- `initialize.params.capabilities.tasks`
- `initialize.params.extensions[]` entries, including objects with `id` or `name`
- request envelope `_meta` and `tools/call.params._meta` for the same shapes, where hosts expose metadata there

The stdio runtime is one logical client session per process. Implementation may keep a process-lifetime derived readiness snapshot in memory so later `doctor` calls can report what was observed during `initialize` or request metadata. It MUST store only normalized classification fields and bounded indicator strings; it MUST NOT persist raw client metadata to the state directory, logs, or transcripts.

Alternatives considered:

- Advertise server task support and see whether clients use it. Rejected because it would claim unsupported behavior.
- Add protocol task methods that always return method-not-found. Rejected because the existing MCP method-not-found behavior already covers unsupported methods.

### Decision: Expose diagnostic classification, not execution

The readiness surface is an additive `doctor.taskExtensionReadiness` section. It should return a small structured classification with observed extension identifiers, legacy indicators, recommended next step, and a boolean `serverAdvertisesTasks: false`.

Rationale: Future implementation decisions need normalized evidence, but clients must not confuse diagnostics with support.

Alternatives considered:

- Put classification only in prose guidance. Rejected because fixtures and future code need stable fields.
- Add a dedicated readiness tool. Rejected for this slice because it adds a new tool when `doctor` is already the setup/readiness surface.
- Expose dynamic diagnostics as a resource. Rejected because current resources are static guidance.
- Persist all observed client metadata. Rejected for privacy and unnecessary state growth; readiness can be request-scoped.

The diagnostic shape is:

```json
{
  "classification": "extension_capable",
  "serverAdvertisesTasks": false,
  "source": "initialize",
  "observedExtensionIdentifiers": ["io.modelcontextprotocol/tasks"],
  "legacyIndicators": [],
  "unknownIndicators": [],
  "recommendedNextStep": "Use Agent Bridge task_* tools; protocol task support is not advertised yet.",
  "checkedAt": "2026-06-05T10:00:00.000Z"
}
```

Classification values:

- `unavailable`: no task-related metadata was observed.
- `extension_capable`: the client declared `io.modelcontextprotocol/tasks`; this is client-shape evidence only because `serverAdvertisesTasks` remains false.
- `legacy_only`: legacy `tasks` capability or 2025-11-25-style task metadata was observed without the current extension identifier.
- `unknown`: task-like metadata was observed but does not match known legacy or current extension shapes.
- `unsupported`: reserved for future cases where the client requests or requires protocol task behavior that Agent Bridge explicitly does not implement. This slice returns method-not-found for `tasks/*`; doctor can remain `unavailable`, `extension_capable`, `legacy_only`, or `unknown` unless unsupported metadata is observed in request metadata.

### Decision: Include legacy and extension fixtures

Tests should cover:

- no task metadata
- current extension metadata:
  ```json
  {
    "capabilities": {
      "experimental": {
        "io.modelcontextprotocol/tasks": {
          "version": "2026-03-26"
        }
      }
    }
  }
  ```
- legacy metadata:
  ```json
  {
    "capabilities": {
      "tasks": {
        "list": true,
        "cancel": true
      }
    }
  }
  ```
- extension-list metadata:
  ```json
  {
    "extensions": [
      {
        "id": "io.modelcontextprotocol/tasks",
        "version": "2026-03-26"
      }
    ]
  }
  ```
- unknown task-like metadata:
  ```json
  {
    "capabilities": {
      "experimental": {
        "taskQueue": true
      }
    }
  }
  ```
- conflicting metadata: current extension wins over legacy when both appear

Rationale: The blocked compatibility memo specifically identified migration differences, so fixtures need to prevent accidental conflation.

### Decision: Reconcile with `explore-mcp-task-support`

This probe change lands before `explore-mcp-task-support` implements protocol task methods. Any host-compatibility overlap should be interpreted as a staged contract: readiness probes observe client metadata and keep server task support unavailable; the later task-support change can build on the readiness classifier when it chooses a negotiated method surface.

## Risks / Trade-offs

- Hosts may not expose per-request capabilities in the same place -> Keep parser tolerant and classify unknown shapes without failing ordinary requests.
- Diagnostic fields may be mistaken for support -> Include `serverAdvertisesTasks: false` and guidance that task execution still uses `task_*`.
- Future extension names may change -> Keep unknown extension identifiers visible in diagnostics rather than hard-failing.

## Migration Plan

1. Add the readiness classifier and fixtures.
2. Expose diagnostics through a narrow diagnostic surface or existing setup/guidance output.
3. Keep all protocol task support disabled.
4. Use collected fixture evidence to decide a later implementation change.
