---
date: 2026-06-16
topic: lean-subagent-contract
type: requirements
---

# Lean Subagent Contract Requirements

## Summary

Agent Bridge should make lean delegation the only provider-facing contract. Spawned subagents receive bounded task input and return only the output needed for the task: no source echo, no progress narration, no generic checklists, and no polish waiting after useful signal is available. Raw logs, transcripts, diffs, and diagnostics remain available through explicit bridge evidence tools.

---

## Problem Frame

Agent Bridge already keeps its own tool responses lean and makes raw evidence opt-in. The remaining context risk is the provider-facing prompt contract: spawned agents can still produce broad final reports because the bridge asks for summary, evidence, changed files, risks, and next steps even when a task needs less.

The goal is to stop context bloat at the source. The caller should get a useful final answer without carrying boilerplate sections, source excerpts that were only inspected as evidence, repeated process narration, or raw evidence that can be fetched on demand.

---

## Key Decisions

- **Only contract, not a default.** Agent Bridge must not offer a verbose provider-final-report mode that bypasses the lean contract.
- **Provider prompt contract over post-processing.** The bridge constrains what the subagent is asked to return; it does not rewrite provider prose into a summary after completion.
- **Evidence remains opt-in.** Raw logs, transcript events, diffs, and detailed diagnostics stay behind existing explicit evidence requests.

---

## Requirements

**Lean Input**

- R1. The bridge must render every provider task prompt with only the task envelope, safety boundary, caller instruction, and lean return contract needed for that task.
- R2. The bridge must avoid adding workflow guidance, diagnostic instructions, or evidence-reading instructions to provider prompts unless that content is necessary for the delegated task itself.
- R3. The bridge must preserve caller-supplied task detail; lean input must remove bridge boilerplate, not truncate the user's actual instruction.

**Lean Output**

- R4. The provider-facing return contract must require the subagent to return only task-relevant final output.
- R5. The provider-facing return contract must explicitly forbid source echo, progress narration, generic checklists, speculative polish, and restating the prompt unless the caller asked for that artifact.
- R6. The provider-facing return contract must require omitted sections when there is no changed file, evidence, risk, blocker, or next step to report.
- R7. The provider-facing return contract must support implementation tasks without padding: changed files, verification evidence, risks, and next steps appear only when they exist.
- R8. The bridge must not expose a launch profile, mode, or option that asks a provider for a verbose final report instead of the lean contract.

**Evidence Boundaries**

- R9. The bridge must keep full stdout, stderr, transcript, diff, and detailed diagnostic material available through explicit `agent_observe` or `agent_result` evidence requests.
- R10. The bridge must continue treating provider output as evidence, not proof; caller-owned verification remains required before claiming work complete.
- R11. The lean provider contract must not change task lifecycle storage or remove raw evidence artifacts.

---

## Acceptance Examples

- AE1. **Covers R1, R2, R4.**
  - **Given:** a caller spawns a research task asking for a short codebase finding.
  - **When:** the provider receives the rendered task prompt.
  - **Then:** the prompt asks for only the bounded finding needed for that task, not a full delegation workflow recap.

- AE2. **Covers R6, R7.**
  - **Given:** a provider completes a task with no file changes and no known risks.
  - **When:** it returns its final answer.
  - **Then:** the answer omits changed-file and risk sections instead of reporting empty placeholders.

- AE3. **Covers R8, R9, R11.**
  - **Given:** a caller needs full logs or transcript evidence.
  - **When:** the caller inspects the task through Agent Bridge.
  - **Then:** the caller requests explicit evidence sections instead of spawning the provider under a verbose-output mode.

- AE4. **Covers R4, R5.**
  - **Given:** a planning or review subagent inspected source material.
  - **When:** it returns its final answer.
  - **Then:** it returns only findings or decision signal, without echoing inspected source or narrating polling, waiting, or polishing.

---

## Scope Boundaries

- No new lifecycle tools or evidence storage.
- No provider-output summarizer in Agent Bridge.
- No alternate verbose provider-final-report profile.
- No replacement for the existing universal output validation work.

---

## Sources / Research

- `crates/agent-bridge-mcp/src/provider.rs` currently renders provider prompts with fixed final-report sections.
- `crates/agent-bridge-mcp/src/task.rs` keeps raw stdout, stderr, diff, and transcript evidence behind explicit result sections.
- `crates/agent-bridge-mcp/src/task/review.rs` builds the review packet and recommended actions from stored task evidence.
- `crates/agent-bridge-mcp/src/guidance.rs` already tells callers that Agent Bridge responses are lean by default and raw evidence is opt-in.
- `openspec/changes/improve-delegation-output-quality` covers provider output validation, retry, and partial-result surfacing rather than the provider-facing lean prompt contract.
