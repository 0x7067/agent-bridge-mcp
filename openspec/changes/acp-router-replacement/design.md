## Context

The current binary is an MCP stdio server. It exposes eight tools and delegates
provider work through a task manager. ACP already exists internally as the child
provider protocol in `task/acp.rs`, but Agent Bridge does not yet expose an
ACP-facing router contract.

The router should not duplicate process supervision, worktree handling, redaction,
or transcript capture. The shortest safe path is to add a router layer that uses
the existing task lifecycle internally until runtime behavior proves a lower-level
extraction is worth owning.

## Goals / Non-Goals

**Goals:**
- Expose an ACP-facing routed prompt-turn contract.
- Keep v1 routing limited to Codex and Claude.
- Preserve workspace confinement, worktree isolation, transcript evidence,
  diagnostics, and provider readiness checks.
- Treat provider-authored completion as trusted finality for the routed turn.
- Make any failover visible in compact diagnostics and evidence references.
- Keep fake-provider tests deterministic and independent of live credentials.

**Non-Goals:**
- Do not add a ninth MCP tool.
- Do not build a multi-provider synthesis fabric.
- Do not silently ask a second provider after a refusal, cancellation, auth
  blocker, billing blocker, or completed answer.
- Do not remove the MCP lifecycle compatibility surface in this change.
- Do not claim provider completion verifies the caller's project-level work.

## Decisions

### D1: Add an explicit ACP router runtime path

The default binary behavior remains the MCP server. The router is exposed through
an explicit runtime path such as `agent-bridge-mcp acp-router` so tests can prove
the new contract before defaults move.

This avoids smuggling the replacement contract into the existing eight-tool MCP
surface and keeps stdout framing testable for each protocol path.

### D2: Reuse the task manager for provider attempts

Each routed attempt may be represented by a normal task record. The router builds
internal spawn arguments, waits for finality, and reads the final record/review
packet through typed helpers.

This preserves worktree isolation, transcript artifacts, provider diagnostics,
cleanup behavior, and existing fake-provider coverage. A direct provider runner
can be extracted later only if the task compatibility layer becomes a measurable
bottleneck.

### D3: Classify attempts before fallback

Router policy maps lower-level evidence into one disposition:

- `TrustedFinal`: provider-authored final answer exists; stop routing.
- `FailoverEligible`: infrastructure, readiness, or lifecycle failure happened
  before trusted finality; policy may try the fallback provider.
- `Blocker`: auth, billing, user cancellation, explicit refusal, or equivalent
  semantic stop; return it without fallback.
- `TerminalFailure`: non-retryable failure that is not a user-facing blocker.

Every fallback records source provider, target provider, reason, failure category,
and attempt ids.

### D4: Keep compact router results evidence-backed

Normal router results contain selected provider, terminal kind, final text or
blocker/failure message, attempts, failover trail, evidence references, and bounded
diagnostics. They do not embed raw stdout, stderr, transcript, or diff bodies.

During migration, evidence references point at the existing task result and
transcript inspection paths.

## Migration Plan

1. Add this OpenSpec change and validate it.
2. Add router domain and pure policy classification tests.
3. Add an internal routed-turn executor that uses `TaskManagerHandle`.
4. Add explicit ACP router stdio runtime handling for `initialize`, `session/new`,
   and `session/prompt`.
5. Add compact router diagnostics and fake-provider coverage for finality,
   failover, blockers, and retained evidence.
6. Update docs to describe the ACP router as the replacement contract and the
   MCP lifecycle as migration compatibility.

## Risks / Trade-offs

- ACP server-side compatibility is new for this repo. Mitigation: keep it behind
  an explicit runtime path and cover stdout framing with stdio tests.
- Failure classification can drift from task diagnostics. Mitigation: introduce
  typed router dispositions and promote ACP stop reasons into diagnostics.
- Reusing task lifecycle internally is not the purest architecture. Mitigation:
  it is the smallest reliable path and preserves proven safety primitives.
- Running MCP and ACP paths side by side can be confused for a permanent dual
  bridge. Mitigation: docs name MCP as migration compatibility.
