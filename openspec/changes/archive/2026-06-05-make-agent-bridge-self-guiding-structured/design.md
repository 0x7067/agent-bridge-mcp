## Context

Agent Bridge currently exposes a provider-neutral `task_*` lifecycle, `doctor`, runtime guidance prompts/resources, presentation metadata, and review packets. The lifecycle is operationally strong, but a caller still needs to remember a recipe spread across README guidance, tool descriptions, and task-result prose.

Current Codex MCP documentation says Codex reads MCP `initialize.instructions` as server-wide guidance, while current MCP tool docs support structured tool content and output schemas. Agent Bridge still initializes without instructions and returns JSON payloads as serialized text content. This change uses those portable surfaces to reduce caller choreography without adding a new orchestration abstraction.

## Goals / Non-Goals

**Goals:**

- Put the safest Agent Bridge workflow in initialization instructions so Codex can use it before choosing tools.
- Keep the first 512 characters of instructions self-contained and focused on the most important workflow and verification boundary.
- Add `structuredContent` to JSON-returning lifecycle tools while preserving the existing text content for clients that depend on it.
- Add output schemas for stable, high-value tool result shapes where the schema can be maintained without overfitting volatile diagnostic payloads.
- Add ranked next-action metadata with ready-to-call tool names and arguments.
- Clarify `doctor` so setup health and launch readiness are not conflated.

**Non-Goals:**

- Do not replace the existing `task_*` lifecycle tools.
- Do not make provider completion count as project verification.
- Do not implement MCP protocol-level tasks in this change.
- Do not require clients to render `structuredContent`, output schemas, or initialization instructions.
- Do not add third-party dependencies.

## Decisions

### Decision: Add initialization instructions without bumping protocol yet

The server should return an `instructions` string from `initialize` while keeping the existing protocol version unless compatibility testing proves a version bump is required.

Rationale: Codex explicitly consumes `instructions`, and the field is additive for clients that tolerate unknown result fields. This provides immediate model guidance with much less risk than adopting experimental task protocol primitives.

Alternatives considered:

- Rely only on prompts/resources. Rejected because prompts are user-selected and resources are application-driven; neither reliably informs model tool selection.
- Bump directly to a newer MCP version. Deferred because protocol negotiation and host compatibility deserve a separate compatibility pass.

### Decision: Add structured content alongside text content

`tool_json` should continue returning the pretty JSON text block and also include `structuredContent` with the original JSON value. Tool definitions should add `outputSchema` for core stable tools where useful.

Rationale: Existing tests and clients parse the text block today. `structuredContent` improves model/client parsing without breaking that path.

Alternatives considered:

- Replace text content with structured content only. Rejected as unnecessarily breaking.
- Add output schemas for every nested diagnostic field. Rejected because diagnostics are intentionally flexible and provider-specific.

### Decision: Next actions are advisory and derived

Task presentation and result surfaces should include a ranked `nextActions` array derived from task state, result inspection state, transcript availability, worktree state, and error type. Each action should include an id, tool name, arguments, reason, and safety state. The first array item is the primary recommendation.

Rationale: The current `presentation.actions` list is useful for UI affordances, but it does not tell callers what to do first. A derived next action preserves explicit tool calls while reducing model choreography mistakes.

Alternatives considered:

- Add a `task_continue` or `task_auto` tool. Rejected because it would hide important inspection and verification gates behind a new automation surface.
- Put next actions only in prose. Rejected because ready-to-call arguments are the main polish gain.

### Decision: Doctor reports setup status and launch readiness separately

`doctor.summary.status` should continue reporting setup blockers, while doctor output should add launch-readiness guidance when selected providers are version-available but not startup-verified. Recommendations should mention `providers_check` with `smoke: true` for that condition.

Rationale: A green doctor with `launchable: false` is technically accurate but ambiguous. Separate readiness avoids turning version-only checks into false confidence.

Alternatives considered:

- Make version-only provider checks a doctor warning. Rejected because it would make default doctor noisy for users who only want setup diagnostics.
- Run smoke by default. Rejected because smoke checks can be slow, consume model/provider resources, and intentionally remain opt-in.

## Risks / Trade-offs

- Some clients may ignore `initialize.instructions` -> Keep prompts/resources and README guidance aligned as fallbacks.
- Output schemas may drift from result payloads -> Keep schemas focused on stable top-level fields and cover them with stdio tests.
- Next actions could imply automation is safe -> Include safety state and keep cleanup/verification boundaries explicit.
- Doctor could become noisy -> Add readiness recommendations only when a caller selected providers or a readiness state is materially actionable.

## Migration Plan

1. Add instructions and structured content behind additive response fields.
2. Add top-level output schemas for stable tools and update stdio fixtures.
3. Add derived next-action metadata for task status/list/result surfaces.
4. Refine doctor readiness fields/recommendations.
5. Update README, prompts, and resources to match.
6. Rollback by ignoring additive fields or reverting the response-shaping changes; existing `task_*` tools remain usable.

## Implementation Constraints

- Use the same `nextActions` array shape on presentation and result/review-packet surfaces.
- Add first-slice output schemas only for stable top-level lifecycle outputs: `doctor`, `task_list`, `task_status`, `task_wait`, and `task_result`.
- Leave provider-specific nested diagnostics modeled as flexible containers until implementation evidence shows a stable schema boundary.
