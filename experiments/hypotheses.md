# Hypothesis Log

## Active Hypotheses
- H01: Shorter initialization guidance can preserve the eight-tool flow and safety rules while cutting `initialize_bytes`.
- H02: Tool descriptions can lose migration-history prose once schemas and docs carry compatibility details, reducing `tools_list_bytes`.
- H03: Prompt and resource bodies repeat the same workflow; compacting repeated wording should reduce `prompts_bytes` and `resources_bytes`.
- H04: Schema property descriptions are longer than needed for LLM callers; targeted shortening should reduce `tools_list_bytes` without changing schema shape.
- H05: Dry-run payload text may include verbose diagnostics that can be shortened without reducing launch safety.

## Closed Hypotheses
- H01 kept in run 2: shortened initialization guidance reduced `initialize_bytes` by 267 bytes with protocol tests passing.
- H03 partial keep in run 3: compacted the review prompt for a 186-byte `prompts_bytes` reduction.
- H03 partial keep in run 4: compacted the implementation prompt for another 261-byte `prompts_bytes` reduction.
- Baseline: total footprint is 54498 bytes; biggest buckets are `providers_list_bytes` 17252 and `resources_bytes` 15126.

- Run 5 keep: docs: shorten result inspection prompt; The explicit section names are enough; the old bullet list duplicated result schema detail. Metric 53445 bytes.

- Run 6 keep: docs: shorten stalled recovery prompt; Safety tests allow concise text as long as the required inspection terms remain. Metric 53077 bytes.

- Run 7 keep: docs: shorten claude host prompt; Lifecycle guidance can be terse because detailed socket behavior is also in resources. Metric 52742 bytes.

- Run 8 keep: docs: shorten dogfood workflows prompt; The detailed dogfood resource can carry nuance; prompt text only needs route selection. Metric 52360 bytes.

- Run 9 keep: docs: shorten provider comparison prompt; Comparison guidance mostly needed wording compression, not structural change. Metric 52026 bytes.

- Run 10 keep: docs: shorten caller workflow resource; Resource prose has much bigger wins than individual prompt bodies. Metric 50147 bytes.

- Run 11 keep: docs: tighten safety resource; Safety prose can be shorter when tests pin the critical diagnostic vocabulary. Metric 49848 bytes.

- Run 12 keep: docs: shorten provider capabilities resource; Runtime providers_list carries detailed capabilities, so static guidance can point to it. Metric 48959 bytes.

- Run 13 keep: docs: shorten claude host resource; Resource and prompt versions can share terse lifecycle language. Metric 48662 bytes.

- Run 14 keep: docs: shorten dogfood workflows resource; Dogfood guidance can be concise without losing the four reproducible paths. Metric 48077 bytes.

- Run 15 keep: docs: shorten code execution resource; Schemas already expose field lists; guidance should teach usage choices. Metric 47547 bytes.

- Run 16 keep: docs: shorten tool descriptions; Tool descriptions were a strong low-risk tools/list target. Metric 46754 bytes.

- Run 17 keep: docs: trim spawn and doctor schema descriptions; Schema descriptions are useful only where they disambiguate behavior. Metric 46255 bytes.

- Run 18 keep: docs: trim observe and result schema descriptions; Top-level tool descriptions plus enums cover most schema semantics. Metric 45712 bytes.

- Run 19 keep: docs: shorten guidance list descriptions; List metadata is loaded before the body, so short labels are enough. Metric 45170 bytes.

- Run 20 keep: docs: shorten provider cadence notes; Capability payload has safe wins in repeated explanatory note strings. Metric 44858 bytes.

- Run 21 keep: docs: shorten antigravity enforcement note; Provider caveats can be concise if the state field remains explicit. Metric 44722 bytes.

- Run 22 keep: docs: shorten profile diagnostic notes; Dry-run payload has smaller but still measurable note-string savings. Metric 44678 bytes.

- Run 23 keep: docs: shorten tool annotation titles; UI labels can be terse because tool names carry context. Metric 44586 bytes.
