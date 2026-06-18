# Autoresearch Worklog: agent bridge cost efficiency

Started: 2026-06-18 00:54 -03

## Data Summary
The benchmark measures actual compact JSON response bytes from the local stdio
server for static MCP discovery/guidance flows and one `agent_spawn` dry run.
The primary metric is `total_bytes` lower-is-better.

## Runs

### Run 1: baseline - total_bytes=54498 (KEEP)
- Timestamp: 2026-06-18 00:56
- What changed: No product change; measured the setup commit.
- Result: total=54498, initialize=1576, tools=8854, prompts=9982, resources=15126, providers=17252, dryrun=1708.
- Insight: Provider capability JSON dominates the static footprint, then guidance resources and prompts.
- Next: Shorten caller-facing guidance first because it is high volume and low-risk.

### Run 2: docs: shorten initialization guidance - total_bytes=54231 (KEEP)
- Timestamp: 2026-06-18 00:58
- What changed: Condensed `initialize.instructions` while keeping provider choice, evidence, eight-tool flow, and verification markers.
- Result: total=54231, initialize=1309, delta=-267 vs best.
- Insight: Initialization prose had safe redundancy; tests preserved the contract phrases.
- Next: Compact read-only and implementation prompt bodies.

### Run 3: docs: tighten review delegation prompt - total_bytes=54045 (KEEP)
- Timestamp: 2026-06-18 00:59
- What changed: Rewrote the read-only review prompt as a shorter flow with the same required tool order and safety markers.
- Result: total=54045, prompts=9796, delta=-186 vs best.
- Insight: Prompt bodies have measurable repeated phrasing and can shrink safely under substring tests.
- Next: Apply the same compacting to isolated implementation guidance.

### Run 4: docs: tighten implementation delegation prompt - total_bytes=53784 (KEEP)
- Timestamp: 2026-06-18 00:59
- What changed: Compacted isolated implementation flow while preserving worktree cleanup and caller verification.
- Result: total=53784, prompts=9535, delta=-261 vs best.
- Insight: Implementation guidance had more removable launch prose than review.
- Next: Shorten result-inspection guidance.

### Run 5: docs: shorten result inspection prompt - total_bytes=53445 (KEEP)
- Timestamp: 2026-06-18 01:00
- What changed: Collapsed result-inspection guidance into the default payload, opt-in sections, and caller proof rule.
- Result: total=53445, delta=-339 vs previous best.
- Insight: The explicit section names are enough; the old bullet list duplicated result schema detail.
- Next: Shorten stalled-task recovery guidance next.

### Run 6: docs: shorten stalled recovery prompt - total_bytes=53077 (KEEP)
- Timestamp: 2026-06-18 01:01
- What changed: Compacted stalled-task recovery while preserving the Codex denial diagnostic terms.
- Result: total=53077, delta=-368 vs previous best.
- Insight: Safety tests allow concise text as long as the required inspection terms remain.
- Next: Shorten Claude host lifecycle prompt next.

### Run 7: docs: shorten claude host prompt - total_bytes=52742 (KEEP)
- Timestamp: 2026-06-18 01:01
- What changed: Reduced Claude host lifecycle prompt to the runner, doctor, ping, mismatch, stop, and fallback rules.
- Result: total=52742, delta=-335 vs previous best.
- Insight: Lifecycle guidance can be terse because detailed socket behavior is also in resources.
- Next: Shorten dogfood workflow prompt next.

### Run 8: docs: shorten dogfood workflows prompt - total_bytes=52360 (KEEP)
- Timestamp: 2026-06-18 01:02
- What changed: Compressed dogfood prompt workflows while retaining read-only, implementation, stalled recovery, provider comparison, and agent_list paths.
- Result: total=52360, delta=-382 vs previous best.
- Insight: The detailed dogfood resource can carry nuance; prompt text only needs route selection.
- Next: Shorten provider comparison prompt next.

### Run 9: docs: shorten provider comparison prompt - total_bytes=52026 (KEEP)
- Timestamp: 2026-06-18 01:02
- What changed: Condensed provider comparison steps while retaining readiness, profile, observe/result, evidence, and verification ownership.
- Result: total=52026, delta=-334 vs previous best.
- Insight: Comparison guidance mostly needed wording compression, not structural change.
- Next: Move from prompt bodies to caller workflow resource.

### Run 10: docs: shorten caller workflow resource - total_bytes=50147 (KEEP)
- Timestamp: 2026-06-18 01:03
- What changed: Reduced caller workflow resource details while retaining doctor, agent_spawn, agent_observe, agent_result, reviewPacket, agent_remove, and lean-contract markers.
- Result: total=50147, delta=-1879 vs previous best.
- Insight: Resource prose has much bigger wins than individual prompt bodies.
- Next: Shorten safety resource next, preserving denial diagnostics.

### Run 11: docs: tighten safety resource - total_bytes=49848 (KEEP)
- Timestamp: 2026-06-18 01:04
- What changed: Compacted safety guidance while preserving Codex denial symptom and inspection terms.
- Result: total=49848, delta=-299 vs previous best.
- Insight: Safety prose can be shorter when tests pin the critical diagnostic vocabulary.
- Next: Shorten provider capabilities resource next.

### Run 12: docs: shorten provider capabilities resource - total_bytes=48959 (KEEP)
- Timestamp: 2026-06-18 01:04
- What changed: Reduced provider capability resource to provider env facts, runtime summary routing, readiness notes, and Codex denial handling.
- Result: total=48959, delta=-889 vs previous best.
- Insight: Runtime providers_list carries detailed capabilities, so static guidance can point to it.
- Next: Shorten Claude host lifecycle resource next.

### Run 13: docs: shorten claude host resource - total_bytes=48662 (KEEP)
- Timestamp: 2026-06-18 01:05
- What changed: Compacted Claude host runner lifecycle and socket guidance while retaining doctor, ping, and workspace_policy_mismatch.
- Result: total=48662, delta=-297 vs previous best.
- Insight: Resource and prompt versions can share terse lifecycle language.
- Next: Shorten dogfood workflows resource next.

### Run 14: docs: shorten dogfood workflows resource - total_bytes=48077 (KEEP)
- Timestamp: 2026-06-18 01:05
- What changed: Converted dogfood workflow prose into compact operator recipes while preserving required section names and lean contract marker.
- Result: total=48077, delta=-585 vs previous best.
- Insight: Dogfood guidance can be concise without losing the four reproducible paths.
- Next: Shorten code-execution resource next.

### Run 15: docs: shorten code execution resource - total_bytes=47547 (KEEP)
- Timestamp: 2026-06-18 01:06
- What changed: Removed schema-like field lists from code-execution guidance while preserving sections, out-of-context evidence, lean final output, source echo, and verification markers.
- Result: total=47547, delta=-530 vs previous best.
- Insight: Schemas already expose field lists; guidance should teach usage choices.
- Next: Move to tool description and schema-description reductions.

### Run 16: docs: shorten tool descriptions - total_bytes=46754 (KEEP)
- Timestamp: 2026-06-18 01:07
- What changed: Shortened top-level public tool descriptions while keeping compatibility anchor phrases.
- Result: total=46754, delta=-793 vs previous best.
- Insight: Tool descriptions were a strong low-risk tools/list target.
- Next: Shorten schema property descriptions next.

### Run 17: docs: trim spawn and doctor schema descriptions - total_bytes=46255 (KEEP)
- Timestamp: 2026-06-18 01:08
- What changed: Removed or shortened doctor and spawn property descriptions where enums and field names were sufficient.
- Result: total=46255, delta=-499 vs previous best.
- Insight: Schema descriptions are useful only where they disambiguate behavior.
- Next: Trim observe and result schema descriptions next.

### Run 18: docs: trim observe and result schema descriptions - total_bytes=45712 (KEEP)
- Timestamp: 2026-06-18 01:08
- What changed: Shortened observe/result property descriptions while preserving enum and bounds constraints.
- Result: total=45712, delta=-543 vs previous best.
- Insight: Top-level tool descriptions plus enums cover most schema semantics.
- Next: Look for structural schema byte reductions next.

### Run 19: docs: shorten guidance list descriptions - total_bytes=45170 (KEEP)
- Timestamp: 2026-06-18 01:09
- What changed: Shortened prompts/list and resources/list descriptions while leaving detailed bodies intact.
- Result: total=45170, delta=-542 vs previous best.
- Insight: List metadata is loaded before the body, so short labels are enough.
- Next: Evaluate prompt definition arguments field next.

### Run 20: docs: shorten provider cadence notes - total_bytes=44858 (KEEP)
- Timestamp: 2026-06-18 01:10
- What changed: Shortened provider outputCadence notes without changing cadence classifications or timing fields.
- Result: total=44858, delta=-312 vs previous best.
- Insight: Capability payload has safe wins in repeated explanatory note strings.
- Next: Shorten Antigravity read-only enforcement note next.

## Key Insights
- Provider capability JSON is the largest bucket, but guidance/resources/prompts are safer first targets.
- Shortening initialization guidance directly lowers every MCP initialization.
- Review prompt compaction preserved behavior and lowered `prompts_bytes`.
- Implementation prompt compaction produced the largest prompt win so far.

## Next Ideas
- Shorten repeated tool descriptions while preserving the eight-tool mapping.
- Remove duplicated guidance between prompts and resources where one reference is enough.
- Keep safety and caller-owned verification wording explicit.
