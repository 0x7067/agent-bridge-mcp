## 1. Reduced-Profile Spike

- [x] 1.1 Create a spike note at `openspec/changes/collect-task-transcripts-and-bare-profiles/implementation.md` with a provider matrix for Claude, Codex, Cursor, and Kimi/Pi.
- [x] 1.2 For each provider, inspect local CLI help/source or reliable local behavior to identify flags/env/config strategies for compact prompts, custom system prompts, hooks, skills/rules, context files, memory/session reuse, config isolation, and auth preservation.
- [x] 1.3 Run tiny preview/smoke probes where safe to confirm which reductions actually work and what evidence proves they worked.
- [x] 1.4 Record each provider capability as `supported`, `unsupported`, or `best_effort`, with exact flags/env/config behavior and caveats.
- [x] 1.5 Record provider CLI versions and the validation method for each spike finding so future provider upgrades have a concrete re-run checklist.
- [x] 1.6 Update the design if the spike shows that `bare` needs a different name, weaker guarantee, or provider-specific fallback behavior.

## 2. Transcript Storage

- [x] 2.1 Add a transcript event model with timestamp, source, provider, event kind, raw text, parsed metadata, and redaction status.
- [x] 2.2 Write `transcript.jsonl` under each task directory while preserving existing stdout and stderr logs.
- [x] 2.3 Capture lifecycle events such as spawn, first output, final output, timeout, stop, provider exit, and finalization.
- [x] 2.4 Add best-effort provider parsers for known structured output, starting with Codex JSONL and preserving raw events for unknown lines.
- [x] 2.5 Redact known prompt bodies, rendered prompts, configured secrets, and provider env values before writing transcript events.
- [x] 2.6 Ensure transcript parse/write failures are reported as diagnostics and do not change provider lifecycle success or failure.

## 3. Transcript API

- [x] 3.1 Add a bounded public transcript inspection surface, either `task_transcript` or an explicitly documented transcript mode on an existing lifecycle tool.
- [x] 3.2 Support cursor or limit arguments so callers can inspect transcript events incrementally.
- [x] 3.3 Redact prompt bodies, rendered prompts, configured secrets, and provider environment values from public transcript responses.
- [x] 3.4 Add tests for transcript event capture, cursor reads, missing transcript behavior, malformed provider output, and redaction.

## 4. Final And Partial Result Detection

- [x] 4.1 Detect provider final-result markers from transcripts for providers with structured final output.
- [x] 4.2 Detect partial-result evidence when provider output contains progress but no complete final result.
- [x] 4.3 Add task result diagnostics such as `finalResultDetected` and `partialResultDetected` without treating provider prose as verification.
- [x] 4.4 Add regressions for stopped or timed-out tasks that emitted a complete provider final result before termination.
- [x] 4.5 Add provider-specific transcript fixtures for Codex JSONL, Claude/Cursor final-result JSON, Kimi/Pi text output, malformed output, and false-positive result-like text.

## 5. Launch Profile Contract

- [x] 5.1 Add a launch profile enum/input field to task preview and spawn validation.
- [x] 5.2 Decide and document the default profile behavior; do not preserve old defaults if an explicit profile contract is clearer.
- [x] 5.3 Persist selected launch profile and profile diagnostics on task records and task results.
- [x] 5.4 Extend `providers_list` with provider launch-profile capability metadata from the spike.
- [x] 5.5 Extend `task_preview` to show selected profile, prompt strategy, applied reductions, unsupported reductions, and best-effort notes.

## 6. Bare Profile Implementation

- [x] 6.1 Implement compact prompt rendering for `bare` profile with minimal mode, cwd, safety, final-report, and user-instruction content.
- [x] 6.2 Implement provider-specific reduced configuration behavior according to the spike findings.
- [x] 6.3 Preserve adapter ownership of profile behavior; do not read provider skill files or introduce runtime skill parsing.
- [x] 6.4 Add fake-provider tests proving `bridge` and `bare` render different prompt/configuration strategies.
- [x] 6.5 Add provider-specific preview tests for applied, unsupported, and best-effort reductions.

## 7. Review Packet And Guidance

- [x] 7.1 Update review packets to include transcript availability, final-result evidence, partial-result evidence, and profile diagnostics.
- [x] 7.2 Update MCP usage guidance to recommend transcripts for run analysis and paired `bridge`/`bare` experiments when evaluating Agent Bridge behavior.
- [x] 7.3 Update README examples for transcript inspection and bare-profile preview/spawn.

## 8. Verification

- [x] 8.1 Run focused unit/integration tests for transcripts, launch profiles, provider capability metadata, and review packets.
- [x] 8.2 Run `cargo test`.
- [x] 8.3 Run `cargo clippy --all-targets -- -D warnings`.
- [x] 8.4 Run `cargo fmt --check`.
- [x] 8.5 Run `openspec validate collect-task-transcripts-and-bare-profiles --strict`.
