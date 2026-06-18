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
