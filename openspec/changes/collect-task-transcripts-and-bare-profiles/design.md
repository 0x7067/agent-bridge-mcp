## Context

Task results currently expose capped stdout/stderr, git state, diagnostics, and review packets. That is enough for manual inspection, but it is weak for analyzing provider behavior over time because callers must parse provider-specific logs themselves. Recent dogfooding also showed that provider behavior is affected by ambient instruction systems such as hooks, skills, global config, and long bridge prompts.

Agent Bridge should improve itself as the single integration surface. This change deliberately avoids repo-owned provider skills and keeps provider-specific launch behavior inside provider adapters.

## Goals / Non-Goals

**Goals:**
- Persist a normalized transcript for every task without replacing raw stdout/stderr logs.
- Expose transcript inspection through an MCP tool or additive `task_result` field.
- Support a `bare` launch profile that uses compact bridge-owned task instructions and reduced provider configuration where provider CLIs allow it.
- Make provider reduced-profile support observable in `providers_list`, `task_preview`, `providers_check`, `task_result`, and diagnostics.
- Add a spike phase that empirically maps which providers can disable or bypass hooks, skills, config files, memory, and extra system prompt layers.

**Non-Goals:**
- Do not introduce provider skills, skill runtime parsing, or a second way to operate providers.
- Do not claim `bare` means identical behavior across providers.
- Do not require transcript parsing to succeed for the task itself to succeed.
- Do not preserve backwards compatibility where it prevents a clearer task-launch contract.

## Decisions

### 1. Store Transcript As A Per-Task JSONL Artifact

Each task gets `transcript.jsonl` under its task directory. Events are appended while stdout/stderr are drained and may also be synthesized at finalization.

Minimum event shape:

```json
{
  "ts": "2026-06-01T16:24:10.123Z",
  "source": "stdout",
  "provider": "codex",
  "kind": "provider_event",
  "raw": "...",
  "parsed": {}
}
```

Rationale: JSONL keeps writes append-only, stream-friendly, and easy to cap or cursor. Raw logs remain the forensic source of truth.

Alternatives considered:
- Store only parsed events. Rejected because provider output formats change and parse failures would destroy debugging evidence.
- Add transcript text into `registry.json`. Rejected because registry should stay lifecycle metadata, not large run content.

### 2. Normalize Without Over-Interpreting

Transcript parsing is best-effort. Provider adapters can parse known structured output such as Codex JSONL, Claude/Cursor final-result JSON, or Kimi text envelopes, but unknown lines still become raw line events. Transcript events are redacted before they are written to `transcript.jsonl`, and public transcript reads apply redaction again as a defense-in-depth check. Existing stdout/stderr logs remain the forensic raw-log artifacts and keep their current access behavior.

Rationale: The transcript should support analysis and final-result detection without turning provider prose into verification claims.

### 3. Add Launch Profiles To Task Spawn/Preview

Add a launch profile field, initially:

- `bridge`: current behavior, using the existing rendered bridge prompt and normal provider configuration.
- `bare`: compact bridge-owned instruction wrapper, provider-specific attempts to disable hooks/skills/config where supported, and diagnostic reporting for reductions that are unsupported or best-effort.

The default should be explicitly selected during implementation. Because backwards compatibility is not required, implementation may make `bridge` explicit in schemas or choose a new default if that improves caller clarity.

Rationale: Profiles keep this inside Agent Bridge's existing provider-adapter abstraction rather than creating separate providers or external skills.

### 4. Bare Profile Is Capability-Reported, Not Assumed

Every provider adapter reports reduced-profile capabilities discovered by the spike:

- compact prompt supported
- custom system prompt supported
- hooks disabled
- skills disabled
- config isolation supported
- memory/context files disabled
- environment minimization supported
- unsupported reductions and best-effort notes

Rationale: "No hooks, no skills" is provider-specific. The MCP caller must see whether `bare` was exact, partial, or unsupported.

### 5. Spike Before Locking Provider Flags

The first implementation phase is a spike that directly tests Claude, Codex, Cursor, and Kimi/Pi launch modes. The spike should use `task_preview`, tiny live or fake-provider probes, local CLI help/source where available, and transcript inspection to record exact flags, env vars, and behaviors.

Questions the spike must answer for each provider:

- Can the provider accept a compact system or instruction prompt separate from the user prompt?
- Can hooks be disabled through flags, env, isolated home/config directories, or a no-config mode?
- Can skills/rules/context files be disabled?
- Can memory/session reuse be disabled?
- Does reduced config still preserve auth?
- What evidence proves the reduction worked?

The spike output should be committed as `implementation.md` or an equivalent design note in this change before provider-specific `bare` behavior is implemented. It must record the provider CLI versions tested and the validation method used for each finding so future provider upgrades have a concrete re-run checklist.

## Risks / Trade-offs

- Transcript files may grow large -> cap public reads and keep raw logs capped; optionally cap stored transcript by event count/bytes in a later change.
- Redaction can miss provider-specific secret formats -> reuse existing redaction sources and add targeted regression tests for prompt/env leakage.
- `bare` may be misleading if reductions are partial -> expose `profileDiagnostics` and provider capability metadata on every preview/result.
- Provider CLI flags may change -> keep the spike output close to adapter tests, record provider versions, and add a maintenance note to rerun the spike when supported provider CLI versions change.
- Comparing `bridge` and `bare` may encourage treating provider output as proof -> keep review packets as evidence summaries and require main-thread verification.

## Migration Plan

1. Add transcript artifacts and public transcript inspection without changing task execution behavior.
2. Add launch-profile schema, preview metadata, and adapter capability reporting.
3. Run the reduced-profile spike and document provider-specific findings.
4. Implement `bare` behavior provider by provider with tests for supported and best-effort cases.
5. Update guidance to recommend paired `bridge`/`bare` experiments when evaluating Agent Bridge behavior.

## Open Questions

- Should transcript inspection be a new `task_transcript` tool, an extended `task_logs` mode, or both?
- Should `bare` be the default for research/review dogfooding tasks, or should callers always select it explicitly?
- Should transcript final-result detection affect task status, or only add diagnostics such as `finalResultDetected`?
