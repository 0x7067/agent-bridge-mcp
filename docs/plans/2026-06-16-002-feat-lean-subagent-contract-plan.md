---
title: "feat: Standardize lean subagent contract"
type: feat
date: 2026-06-16
origin: docs/brainstorms/2026-06-16-lean-subagent-contract-requirements.md
---

# feat: Standardize lean subagent contract

## Summary

Make the provider-facing delegation contract lean-only and explicit about forbidden output. Spawned agents should receive bounded task context and return only task-relevant final output: no source echo, no progress narration, no generic checklists, and no waiting for polish when the useful signal is already available.

## Problem Frame

Agent Bridge already keeps MCP tool responses lean and evidence sections opt-in, but provider task prompts still ask for broad final reports. That leaks context-window cost into every delegated run, encourages agents to echo source material, and leaves "lean" as a profile flavor instead of the only delegation contract.

## Requirements

- R1. Every spawned provider task receives a prompt with the task envelope, safety boundary, caller instruction, and lean return contract.
- R2. The lean return contract explicitly forbids source echo, progress narration, generic checklists, speculative polish, and restating the prompt unless the caller asked for that artifact.
- R3. Providers return the smallest final answer that satisfies the task, or a blocker with the one missing fact needed to proceed.
- R4. Providers include changed files, verification evidence, risks, blockers, and next steps only when those items exist for the task.
- R5. No launch profile, tool input, or documented option reintroduces a verbose provider final-report mode.
- R6. Existing `agent_observe` and `agent_result` evidence access remains the path for raw logs, diffs, transcripts, and diagnostics.
- R7. Provider output remains evidence, not proof; callers still verify locally before claiming completion.

## Key Technical Decisions

- KTD1. Contract in prompt rendering: provider compliance starts with the prompt text, not a post-processing summarizer.
- KTD2. One contract across launch profiles: profiles may change provider launch configuration, but not the expected final-answer shape.
- KTD3. Keep evidence storage untouched: the existing result-section plumbing already gives callers raw evidence on demand.
- KTD4. Negative contract is part of the contract: tests should protect the "do not echo, narrate, or pad" rules, not just the positive summary shape.
- KTD5. Test substrings, not whole prompts: prompt text is user-facing enough to protect, but full-snapshot tests would be brittle.

## High-Level Technical Design

```text
agent_spawn input
  -> provider prompt renderer
  -> ACP stdin / provider launch
  -> provider final output

agent_observe / agent_result sections
  -> raw evidence on explicit request
```

The implementation should make the prompt renderer the single source of truth for the lean provider contract. Result assembly should keep its current opt-in evidence behavior.

## Implementation Units

- U1. Replace broad final-report prompt wording with a shared lean return contract in `crates/agent-bridge-mcp/src/provider.rs`.
  - Preserve caller task text and existing safety boundaries.
  - Apply the same final-output contract to bridge, bare, and unblocked launch profiles.
  - Include explicit banned output classes: source echo, progress narration, generic checklist padding, speculative polish, and prompt restatement.
  - Add provider unit tests for each task mode and profile class that assert the lean contract and banned-output rules are present and the old broad-report wording is gone.

- U2. Keep the public tool surface lean-only.
  - Confirm `agent_spawn` takes no output-verbosity field and rejects unknown fields through the existing strict schema behavior.
  - Update descriptions or diagnostics that imply bare/compact is the only lean mode.
  - Add protocol tests that prevent a verbose final-report option from appearing in tool schema or guidance.

- U3. Prove transport and result boundaries with integration coverage.
  - Use existing fake-provider patterns to capture provider stdin and assert the lean contract reaches the launched provider.
  - Keep dry-run prompt redaction unchanged.
  - Assert default `agent_result` output omits raw stdout, stderr, diff, and transcript while explicit sections still return them.
  - Add a regression case where a planning/review-style task is instructed to return findings only, without echoing source text or narrating polling/progress.

- U4. Update durable guidance and specs that describe delegation behavior.
  - Align docs and OpenSpec text with lean-only provider output and opt-in raw evidence.
  - Tell callers to stop or ignore subagents once useful evidence is available instead of waiting for nonessential polish.
  - Keep the eight-tool lifecycle and local-verification rule intact.

## Scope Boundaries

- No new MCP lifecycle tools.
- No provider-output summarizer or parser.
- No new verbose/compact output option.
- No storage, transcript, or diff retention changes.
- No universal provider-output validation work in this plan.

## Risks & Dependencies

- Providers may still ignore prompt instructions. This plan makes the bridge contract unambiguous, but enforcement beyond prompt wording is separate validation work.
- Over-tight prompt wording could hide useful implementation evidence. The contract should allow evidence when it exists, without requiring empty sections or progress narration.
- Prompt tests can become noisy if they assert full strings. Keep them focused on stable contract phrases.

## Acceptance Examples

- AE1. A research task returns the relevant finding only; it is not asked to include changed files, verification, risks, or next steps when none exist.
- AE2. An implementation task that changes files returns the summary plus actual changed files and verification evidence; it omits empty risk or next-step sections.
- AE3. A caller who needs raw logs or a diff requests the existing evidence sections through `agent_result`; the provider prompt does not preload those details into every final answer.
- AE4. A planning or review subagent that inspected source material returns only the findings or decision signal; it does not echo the inspected source or narrate that it is polling, waiting, or polishing.

## Sources / Research

- `docs/brainstorms/2026-06-16-lean-subagent-contract-requirements.md` defines the lean-only contract and scope boundaries.
- `crates/agent-bridge-mcp/src/provider.rs` renders provider prompts and launch-profile diagnostics.
- `crates/agent-bridge-mcp/src/task.rs` and `crates/agent-bridge-mcp/src/task/review.rs` implement default and opt-in result evidence sections.
- `crates/agent-bridge-mcp/src/tools.rs` and `crates/agent-bridge-mcp/tests/server_protocol.rs` define and test the public MCP schema.
- `crates/agent-bridge-mcp/tests/stdio_binary.rs` contains fake-provider and stdio integration coverage.
- `docs/ADR/0001-consolidate-eight-tools.md`, `docs/agents/architecture.md`, and `docs/agents/guardrails.md` establish the lean eight-tool lifecycle and "evidence, not proof" rule.
- `openspec/specs/provider-adapter-contract/spec.md`, `openspec/specs/delegated-review-packet/spec.md`, and `openspec/specs/mcp-usage-guidance/spec.md` capture the provider contract and result guidance constraints.
