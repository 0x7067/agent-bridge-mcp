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

## Key Insights
- Provider capability JSON is the largest bucket, but guidance/resources/prompts are safer first targets.
- Shortening initialization guidance directly lowers every MCP initialization.

## Next Ideas
- Shorten repeated tool descriptions while preserving the eight-tool mapping.
- Remove duplicated guidance between prompts and resources where one reference is enough.
- Keep safety and caller-owned verification wording explicit.
