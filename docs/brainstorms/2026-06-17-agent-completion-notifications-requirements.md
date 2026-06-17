---
date: 2026-06-17
topic: agent-completion-notifications
type: requirements
---

# Agent Completion Notifications Requirements

## Summary

Agent Bridge should treat spawned agents as watched first-party agents. When a watched agent reaches a final state, the bridge pushes one completion notification to the main agent with enough compact summary for the main agent to proceed without fetching raw evidence by default.

---

## Problem Frame

Agent Bridge already exposes a lean eight-tool lifecycle and progress-aware observation, but the caller still pays a polling tax to notice completed work. A delegated provider can finish while the main agent is working elsewhere, leaving the caller to remember to poll `agent_observe` or scan `agent_list`.

The native-feeling behavior is attention-based. The main agent should be alerted when delegated work finishes, then decide whether to inspect, verify, ignore, stop, or clean up from the compact signal it received.

---

## Key Decisions

- **Push over polling.** Completion should reach the main agent when it happens; repeated observation remains useful for progress and diagnostics, not for noticing finality.
- **Summary included.** The completion notification carries enough final context for the next decision, instead of only saying that an agent finished.
- **One notification per finished agent.** Each watched agent completion gets its own alert; batching is deferred.
- **Inspection clears attention.** A finished agent remains attention-worthy until `agent_result` is inspected.
- **Current-session v1.** The first version watches agents spawned in the current connected session; restart and reconnect catch-up is deferred.

---

## Actors

- A1. **Main agent.** The MCP client agent that delegates work and needs to regain attention when delegated work finishes.
- A2. **Agent Bridge.** The server that owns provider lifecycle state, result evidence, and completion notification delivery.
- A3. **Provider agent.** The local delegated agent process whose completion triggers finalization and notification.

---

## Key Flows

- F1. Completion alert
  - **Trigger:** A watched provider agent reaches a final state.
  - **Actors:** A1, A2, A3
  - **Steps:** Agent Bridge finalizes the agent, builds a compact completion summary, and pushes one notification to the main agent.
  - **Outcome:** The main agent has enough context to choose the next lifecycle action without polling.
  - **Covered by:** R1, R2, R3, R4

- F2. Attention list triage
  - **Trigger:** The main agent asks for active and recent agents.
  - **Actors:** A1, A2
  - **Steps:** Agent Bridge returns active agents first, then finished agents whose results have not been inspected.
  - **Outcome:** `agent_list` behaves like an attention inbox instead of a raw history scan.
  - **Covered by:** R6, R7, R8

- F3. Result inspection clears attention
  - **Trigger:** The main agent inspects a finished agent through `agent_result`.
  - **Actors:** A1, A2
  - **Steps:** Agent Bridge records result inspection and removes that agent from the default attention set.
  - **Outcome:** The agent remains available through filtered/history listing but no longer competes for default attention.
  - **Covered by:** R7, R8

---

## Requirements

**Completion Notification**

- R1. Agent Bridge must watch agents spawned in the current connected session for final lifecycle transitions.
- R2. Agent Bridge must push exactly one completion notification when a watched agent reaches a final state.
- R3. The completion notification must identify the agent, provider, mode, title or fallback display label, final status, completion time, and whether attention is required.
- R4. The completion notification must include a compact final summary sufficient for the main agent to choose the next lifecycle action.
- R5. The notification must not include raw stdout, raw stderr, full transcript events, or full diffs by default.

**Attention Inbox**

- R6. The default `agent_list` view must prioritize active agents first, then final agents whose results have not been inspected.
- R7. Inspecting `agent_result` for a final agent must clear that agent from the default attention set.
- R8. Inspected final agents must remain retrievable through explicit filters or non-default history views.

**Verification Boundary**

- R9. Completion notifications must not claim that delegated work has been verified by the main agent.
- R10. The compact summary may include provider status, changed-file count, result availability, risks, and recommended next actions.
- R11. Raw evidence must remain available through explicit lifecycle inspection.

**Delivery Semantics**

- R12. Notification delivery must be best-effort for the current connected session in v1.
- R13. Missing a notification must not lose the final result; `agent_list` and `agent_result` remain the recovery path.
- R14. Restart and reconnect catch-up notifications are deferred unless planning finds a low-cost protocol-supported path.

---

## Acceptance Examples

- AE1. **Covers R1, R2, R3, R4.**
  - **Given:** the main agent spawns a watched review agent.
  - **When:** the provider agent reaches a final state.
  - **Then:** Agent Bridge pushes one completion notification with identity, final status, completion time, compact summary, and a ready next action.

- AE2. **Covers R4, R5, R10, R11.**
  - **Given:** a provider agent finishes with changed files and transcript evidence.
  - **When:** the completion notification is delivered.
  - **Then:** the notification summarizes what the main agent needs to decide next without embedding raw transcript, logs, or diff content.

- AE3. **Covers R6, R7, R8.**
  - **Given:** one agent is running, one finished agent has not been inspected, and one older finished agent has been inspected.
  - **When:** the main agent calls `agent_list` with default arguments.
  - **Then:** the running agent and uninspected finished agent appear ahead of inspected history.

- AE4. **Covers R9, R10, R11.**
  - **Given:** a provider reports success.
  - **When:** Agent Bridge sends the completion notification.
  - **Then:** the notification says the provider finished and points to verification or result inspection, but does not claim the project is done.

- AE5. **Covers R12, R13, R14.**
  - **Given:** the server restarts after an agent finishes.
  - **When:** the main agent reconnects.
  - **Then:** v1 does not need to replay the missed notification, but the final agent remains discoverable through `agent_list` and inspectable through `agent_result`.

---

## Success Criteria

- The main agent does not need polling solely to notice that a current-session watched agent finished.
- The first completion alert usually contains enough context for the main agent to inspect, verify, or clean up without an exploratory `agent_observe` call.
- Default `agent_list` reads as an attention inbox, not a historical registry dump.
- The verification boundary remains visible in notification and listing behavior.

---

## Scope Boundaries

- No raw evidence payloads in completion notifications by default.
- No batch or coalesced completion alerts in v1.
- No restart or reconnect replay requirement in v1.
- No claim that provider completion equals project verification.
- No new provider-output verbosity mode.

---

## Dependencies / Assumptions

- The host client can receive a server-originated completion signal or an equivalent host-supported push event.
- Agent Bridge can derive the compact summary from already-normalized lifecycle and result metadata.
- Existing result-inspection tracking remains the right acknowledgement point for clearing attention.

---

## Sources / Research

- `docs/ADR/0001-consolidate-eight-tools.md` records the eight-tool lifecycle and the decision to keep responses lean with raw evidence opt-in.
- `crates/agent-bridge-mcp/src/task.rs` already has internal watcher signaling for waits and observations, and finalization writes completion metadata before signaling watchers.
- `crates/agent-bridge-mcp/src/task/review.rs` shapes compact agent-facing status envelopes with progress and next actions.
- `crates/agent-bridge-mcp/src/server.rs` makes default `agent_list` use active/recent presentation mode.
- `crates/agent-bridge-mcp/tests/server_protocol.rs` currently verifies inbound initialized and unknown notifications are ignored; completion push is new behavior, not a current public contract.
