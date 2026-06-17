---
title: "feat: Add Agent Completion Notifications"
type: feat
date: 2026-06-17
origin: docs/brainstorms/2026-06-17-agent-completion-notifications-requirements.md
---

# feat: Add Agent Completion Notifications

## Summary

Add current-session completion notifications for watched Agent Bridge agents. A finalized agent pushes one compact server-to-client notification, and the default `agent_list` becomes an attention inbox for active and finished-uninspected agents.

---

## Problem Frame

Agent Bridge already has compact lifecycle reads, but callers still need to poll or list agents to notice completion. The origin requirements define native-feeling behavior as attention-based: completion should alert the main agent, include enough summary to proceed, and preserve explicit result inspection as the verification boundary.

---

## Requirements

**Completion Notification**

- R1. The server sends one completion notification for each current-session watched agent that reaches a final state.
- R2. The notification identifies the agent, provider, mode, display title, final status, completion time, and attention requirement.
- R3. The notification includes compact final context and next actions without embedding raw stdout, stderr, transcript, or diff bodies.
- R4. Notification delivery is best-effort; missed notifications never remove stored result evidence.

**Attention Inbox**

- R5. Default `agent_list` returns active agents first, then final agents whose result has not been inspected.
- R6. `agent_result` inspection clears a final agent from the default attention set.
- R7. Inspected final agents remain available through explicit filters or full-history listing.

**Verification Boundary**

- R8. Notifications and listing summaries never claim caller-side project verification passed.
- R9. Raw evidence remains opt-in through existing result and transcript inspection paths.

---

## Key Technical Decisions

- **Use a project-specific MCP notification.** Use a server-to-client JSON-RPC notification such as `notifications/agent_bridge/agent_completed`, not `notifications/progress`, because MCP progress notifications are tied to an active request progress token.
- **Emit from the stdio runtime writer.** Keep all stdout protocol writes serialized through the runtime loop so notifications cannot interleave with JSON-RPC responses.
- **Reuse result summary shaping.** Build the notification payload from the same normalized task metadata and review-packet logic used by `agent_result`, with raw evidence omitted.
- **Treat result inspection as acknowledgement.** Keep the existing `result_inspected_at` field as the attention-clearing source of truth.
- **Do not add lifecycle tools.** Preserve the eight-tool surface; completion alerts change delivery, not the tool list.

---

## High-Level Technical Design

```mermaid
sequenceDiagram
  participant Provider as Provider agent
  participant Actor as Task actor
  participant Runtime as Stdio runtime
  participant Main as Main agent

  Provider->>Actor: Final completion
  Actor->>Actor: Finalize task and save result
  Actor->>Runtime: Completion event
  Runtime->>Main: notifications/agent_bridge/agent_completed
  Main->>Actor: agent_result when ready
  Actor->>Actor: Mark result inspected
```

The notification path should be additive to existing `agent_observe` waiters. Existing watches still wake blocked observation calls; the new completion event feeds the stdio writer for current-session push delivery.

---

## Implementation Units

### U1. Notification Protocol Shape

- **Goal:** Define the server-to-client notification envelope and compact completion summary.
- **Requirements:** R1, R2, R3, R8, R9; origin F1, AE1, AE2, AE4.
- **Dependencies:** None.
- **Files:** `crates/agent-bridge-mcp/src/mcp.rs`, `crates/agent-bridge-mcp/src/task/review.rs`, `crates/agent-bridge-mcp/tests/protocol_models.rs`, `crates/agent-bridge-mcp/tests/server_protocol.rs`.
- **Approach:** Add a serializable JSON-RPC notification type with no `id`. Add a helper that creates the completion notification payload from a final task using compact task state, review packet, and next actions. Keep raw evidence fields out of this helper.
- **Patterns to follow:** `JsonRpcResponse` in `crates/agent-bridge-mcp/src/mcp.rs`; `public_task`, `review_packet`, and `next_actions` in `crates/agent-bridge-mcp/src/task/review.rs`.
- **Test scenarios:**
  - Covers AE1. Serializing a completion notification omits `id`, uses `jsonrpc: "2.0"`, and uses the agreed Agent Bridge completion method.
  - Covers AE2. A task with stdout, stderr, transcript, and diff evidence produces a notification payload with review summary fields but no raw evidence bodies.
  - Covers AE4. A successful provider task notification includes provider status and next actions but no project-verification claim.
- **Verification:** Protocol model tests prove notification serialization, and server protocol tests lock the compact payload shape.

### U2. Runtime Completion Delivery

- **Goal:** Deliver one current-session completion notification per watched agent without corrupting stdio JSON-RPC output.
- **Requirements:** R1, R2, R4; origin R1, R2, R12, R13.
- **Dependencies:** U1.
- **Files:** `crates/agent-bridge-mcp/src/runtime.rs`, `crates/agent-bridge-mcp/src/server.rs`, `crates/agent-bridge-mcp/src/task.rs`, `crates/agent-bridge-mcp/tests/stdio_binary.rs`.
- **Approach:** Introduce an internal completion-event channel from task finalization to the stdio runtime. The runtime owns both response and notification writes, so every outbound message remains one newline-delimited MCP JSON object. Register watches for agents spawned through the current server session only.
- **Patterns to follow:** `run_stdio_server` and `write_response` in `crates/agent-bridge-mcp/src/runtime.rs`; task actor completion flow and `signal_task` in `crates/agent-bridge-mcp/src/task.rs`.
- **Test scenarios:**
  - Covers AE1. Spawning a fake provider that completes causes one completion notification line before or alongside subsequent readable responses.
  - Covers AE5. A pre-existing final task from registry startup does not emit a replay notification.
  - A completed task still returns from `agent_result` if the notification is ignored by the client.
  - Multiple current-session agents finishing produce one notification per agent.
- **Verification:** Stdio integration tests parse outbound lines and prove each line is valid JSON-RPC, with no duplicate completion notifications.

### U3. Attention Inbox Listing

- **Goal:** Make default `agent_list` prioritize active agents and finished-uninspected agents while preserving explicit history access.
- **Requirements:** R5, R6, R7; origin F2, F3, AE3.
- **Dependencies:** None.
- **Files:** `crates/agent-bridge-mcp/src/task/review.rs`, `crates/agent-bridge-mcp/src/task.rs`, `crates/agent-bridge-mcp/src/server.rs`, `crates/agent-bridge-mcp/tests/stdio_binary.rs`.
- **Approach:** Update presentation-list sorting and default filtering so active agents remain first, uninspected final agents follow, and inspected final history is excluded from the default attention set unless filters or full-history mode request it. Keep `agent_result` as the only acknowledgement path by relying on `result_inspected_at`.
- **Patterns to follow:** `list_tasks`, `compare_for_presentation_list`, `agent_list_arguments`, and `inspect_result`.
- **Test scenarios:**
  - Covers AE3. Default `agent_list` returns running agents before final-uninspected agents and omits inspected final agents.
  - Calling `agent_result` for a final agent removes it from default `agent_list`.
  - Explicit filters still retrieve inspected final agents when the filter matches.
  - Full-history listing in internal task tests still includes inspected final agents and removed-task exclusions remain intact.
- **Verification:** Unit tests cover sort/filter behavior; stdio tests cover the public `agent_list` wrapper.

### U4. Guidance, Schemas, and Documentation

- **Goal:** Teach callers that completion alerts are the primary finality signal while keeping observation and result tools as recovery and evidence paths.
- **Requirements:** R3, R4, R8, R9; origin success criteria.
- **Dependencies:** U1, U2, U3.
- **Files:** `crates/agent-bridge-mcp/src/guidance.rs`, `crates/agent-bridge-mcp/src/tools.rs`, `docs/DOCUMENTATION.md`, `docs/agents/architecture.md`, `docs/BUSINESS-CONTEXT.md`, `crates/agent-bridge-mcp/tests/server_protocol.rs`.
- **Approach:** Update initialization guidance, prompt/resource guidance, tool descriptions, output schemas, and docs to describe completion notifications, attention-list behavior, and result inspection as acknowledgement. Keep the eight-tool workflow intact.
- **Patterns to follow:** Existing consolidated-surface guidance in `crates/agent-bridge-mcp/src/guidance.rs`; tool schema assertions in `crates/agent-bridge-mcp/tests/server_protocol.rs`.
- **Test scenarios:**
  - Tool descriptions still advertise the eight-tool surface and no removed tools reappear.
  - Initialization instructions mention completion notifications without weakening caller-owned verification.
  - `agent_list` output schema still exposes lean agent summaries and no GUI presentation blob.
- **Verification:** Protocol tests lock guidance/schema changes; documentation review confirms repo-relative references.

---

## Scope Boundaries

- No restart or reconnect notification replay in v1.
- No batch or coalesced completion notifications.
- No raw evidence bodies in completion notifications.
- No new public lifecycle tools.
- No provider-output verbosity mode.

---

## Risks & Dependencies

- **Client support for custom notifications:** MCP allows notifications in either direction, but host clients may ignore unknown notification methods. Mitigation: keep `agent_list` and `agent_result` as recovery paths and document best-effort delivery.
- **Stdout protocol safety:** Notifications share stdout with responses. Mitigation: centralize outbound writes in the runtime and test line-by-line JSON validity.
- **Notification duplication:** Finalization, retries, and stop paths can all produce final states. Mitigation: track current-session notification eligibility and emit once per original finished agent.
- **Attention list compatibility:** Changing default list behavior may surprise consumers expecting recent inspected history. Mitigation: preserve explicit filters or full-history access.

---

## Documentation / Operational Notes

- Update caller guidance to describe completion notifications as the preferred finality signal.
- Note that provider completion is evidence only; the main agent still runs project verification.
- Document that missed notifications are recoverable with `agent_list` and `agent_result`.

---

## Sources & Research

- Origin requirements: `docs/brainstorms/2026-06-17-agent-completion-notifications-requirements.md`.
- MCP specification: JSON-RPC notifications are one-way messages without IDs, and stdio transport permits requests, notifications, and responses over stdout as newline-delimited valid MCP messages: https://modelcontextprotocol.io/specification/2025-06-18/basic and https://modelcontextprotocol.io/specification/2025-06-18/basic/transports.
- MCP progress utility: `notifications/progress` is bound to active request `progressToken`s, so completion alerts should not misuse it: https://modelcontextprotocol.io/specification/2025-06-18/basic/utilities/progress.
- Existing runtime: `crates/agent-bridge-mcp/src/runtime.rs` currently writes only responses through `write_response`.
- Existing lifecycle: `crates/agent-bridge-mcp/src/task.rs` finalizes task metadata, writes result evidence, and signals internal watchers.
- Existing summaries: `crates/agent-bridge-mcp/src/task/review.rs` provides compact public state, next actions, review packets, and list ordering.
