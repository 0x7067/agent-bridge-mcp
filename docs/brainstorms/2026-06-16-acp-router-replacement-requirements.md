---
date: 2026-06-16
topic: acp-router-replacement
type: requirements
---

# ACP Router Replacement Requirements

## Summary

Agent Bridge should move from an MCP lifecycle bridge toward an ACP-only router
that policy-routes prompt turns between provider agents. V1 should focus on
Codex and Claude routing, trusted finality, bounded failover, and preserving
diagnostic evidence without asking clients to orchestrate provider lifecycle
state directly.

---

## Problem Frame

The current Agent Bridge product contract exposes an MCP server with explicit
spawn, observe, result, list, stop, and remove tools. That surface is useful for
bounded delegation, but it still makes the calling client reason about agent
lifecycle state, polling, result inspection, and provider-specific failure
recovery.

The replacement direction is a narrower native-collaboration contract: the
router owns one prompt turn, chooses a provider by policy, streams provider
internals only as debug or evidence updates, and returns a trusted final answer
or a clear failure. Worktree isolation, diagnostics, and transcripts remain
reliability primitives, not the primary user-facing collaboration model.

---

## Key Decisions

- **ACP router, not dual bridge.** Agent Bridge should become an ACP router, not
  a dual MCP-plus-ACP bridge and not a multi-provider synthesis fabric in v1.
- **Codex and Claude first.** V1 policy routing should focus on Codex and
  Claude before broad provider support.
- **Router-owned finality.** The router owns prompt-turn finality instead of
  asking clients to interpret observe/result state.
- **Evidence streams are secondary.** Provider internals should stream as
  debug/evidence updates, not as the main product contract.
- **Bounded failover.** Automatic failover is allowed only for infrastructure,
  readiness, and lifecycle failures before trusted finality.
- **No silent semantic failover.** Auth, billing, user cancellation, explicit
  provider refusal, and completed provider-authored answers must not silently
  fail over to another provider.

---

## Requirements

**Router Contract**

- R1. The public replacement contract must model one routed prompt turn, not a
  caller-managed provider task lifecycle.
- R2. The router must choose between Codex and Claude using explicit policy
  inputs and provider readiness.
- R3. The router must return a final provider-authored answer, a clear blocker,
  or a classified failure for each prompt turn.
- R4. The router must preserve transcript and diagnostic evidence for audit and
  debugging without making clients fetch raw lifecycle state by default.

**Finality and Failover**

- R5. The router must treat a completed provider-authored answer as trusted
  finality for that prompt turn.
- R6. The router may automatically fail over only when the first provider fails
  before trusted finality because of infrastructure, readiness, or lifecycle
  failure.
- R7. The router must not silently fail over on auth failure, billing failure,
  user cancellation, explicit provider refusal, or a completed answer.
- R8. Any failover that occurs must be visible in diagnostics and evidence.

**Preserved Reliability Primitives**

- R9. Workspace confinement and worktree isolation must remain available as
  safety and reliability primitives.
- R10. Provider readiness checks must remain bounded and policy-visible.
- R11. Transcripts, stdout/stderr excerpts, and provider diagnostics must remain
  inspectable for debugging and trust calibration.
- R12. Existing MCP lifecycle behavior may remain during migration, but it must
  not define the final replacement product contract.

---

## Acceptance Examples

- AE1. **Routed prompt turn.**
  - **Given:** both Codex and Claude are ready.
  - **When:** a caller sends a prompt turn to the router.
  - **Then:** the router chooses one provider by policy, streams any provider
    internals as evidence/debug updates, and returns one final answer or
    classified failure.

- AE2. **Infrastructure failover.**
  - **Given:** Codex is selected first but fails to launch before producing a
    trusted final answer.
  - **When:** Claude is ready and the failure category is infrastructure,
    readiness, or lifecycle.
  - **Then:** the router may retry the prompt turn with Claude and records the
    failover in diagnostics.

- AE3. **No semantic failover.**
  - **Given:** Claude returns an explicit refusal or auth/billing failure.
  - **When:** Codex is also available.
  - **Then:** the router returns the classified blocker/failure and does not
    silently ask Codex for an alternate answer.

- AE4. **Evidence retained.**
  - **Given:** a provider turn completes.
  - **When:** a caller needs to audit behavior.
  - **Then:** transcript and diagnostic evidence remain inspectable even though
    the normal contract returned only the prompt-turn result.

---

## Scope Boundaries

- No full multi-provider synthesis fabric in v1.
- No arbitrary prompt-mode provider discovery.
- No silent provider fallback after trusted finality.
- No removal of existing reliability primitives until replacement behavior is
  proven.
- No claim that provider completion verifies the caller's project-level work.

---

## Sources / Research

- Memory decision: [[Agent Bridge ACP-only router replacement requirements]].
- `crates/agent-bridge-mcp/src/server.rs` currently exposes the MCP JSON-RPC
  method dispatcher and eight-tool lifecycle surface.
- `crates/agent-bridge-mcp/src/task/acp.rs` currently owns provider ACP child
  dialogs.
- `crates/agent-bridge-mcp/src/provider.rs` currently owns provider command
  construction, readiness smoke prompts, launch profiles, and provider-specific
  failure detection.
- `docs/brainstorms/2026-06-16-unblocked-provider-discovery-requirements.md`
  preserves ACP-only provider discovery and workspace validation constraints.
