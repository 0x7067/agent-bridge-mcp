# Autoresearch: agent bridge polling friction

## Objective
Reduce Agent Bridge caller-facing polling awkwardness for common delegated-agent
progress loops. The workload checks whether the first running-agent `next`
action recommends quiet finality waiting instead of immediate transcript polling.

## Metrics
- **Primary**: `polling_friction` (points, lower is better; `1` means the
  first running next action still encourages polling, `0` means it waits for
  finality first)
- **Secondary**: `running_first_wait_final`, `running_first_observe`

## How to Run
`./autoresearch.sh` - outputs `METRIC name=number` lines.

## Files in Scope
- `crates/agent-bridge-mcp/src/task/review.rs` - running-agent next action
  ordering and action reasons.
- `crates/agent-bridge-mcp/src/task.rs` - focused unit tests for public task
  and timeline next actions.
- `crates/agent-bridge-mcp/tests/stdio_binary.rs` - protocol-level list output
  assertions.
- `crates/agent-bridge-mcp/src/guidance.rs` - initialization text, prompts, resources.
- `crates/agent-bridge-mcp/src/tools.rs` - public tool descriptions and schema text.
- `crates/agent-bridge-mcp/src/provider.rs` - provider prompt wording only if a clear low-risk prompt-cost win appears.
- `crates/agent-bridge-mcp/tests/server_protocol.rs` - protocol/guidance assertions.
- `crates/agent-bridge-mcp/tests/stdio_binary.rs` - stdio protocol assertions.
- `README.md` and docs that explicitly mirror changed user-facing guidance.
- `autoresearch.md`, `autoresearch.jsonl`, `autoresearch-dashboard.md`,
  `autoresearch.sh`, and `experiments/*` - experiment state and logs.

## Off Limits
- Provider launch architecture, task state storage, ACP protocol behavior, security boundaries.
- New dependencies.
- Removing capabilities from the eight-tool lifecycle.
- Hiding the "provider output is evidence, not proof" rule.

## Constraints
- Lower polling friction only counts when focused tests still pass.
- Prefer deletion and concise wording over abstractions.
- Keep strict schemas and compatibility assertions.
- Discard any reduction that makes guidance ambiguous about verification, raw evidence access, or cleanup safety.

## What's Been Tried
- Run 1 baseline: 54498 bytes. Largest measured buckets are provider capability JSON (17252), resources (15126), prompts (9982), and tools/list (8854).
- Run 2 kept: shortened initialization guidance to 54231 bytes without losing required safety/workflow markers.
- Run 3 kept: tightened read-only review prompt; total 54045 bytes.
- Run 4 kept: tightened implementation prompt; total 53784 bytes.
- Runs 5-9 kept: compacted the remaining prompt bodies (result inspection, stalled recovery, Claude host lifecycle, dogfood, provider comparison), reaching 52026 bytes.
- Runs 10-11 kept: compacted caller workflow and safety resources, reaching 49848 bytes. Resource prose is a larger optimization target than individual prompt bodies.
- Runs 12-15 kept: compacted provider, Claude-host, dogfood, and code-execution resources, reaching 47547 bytes.
- Runs 16-19 kept: shortened tool descriptions, schema descriptions, and guidance list metadata, reaching 45170 bytes.
- Runs 20-26 kept: shortened provider/dry-run diagnostic notes, annotation titles, omitted optional empty prompt arguments and resource-list MIME types, and removed duplicated provider-mode prose. Best is run 26 at 44143 bytes, 19.0% below baseline.
- Segment 1 starts after user steer: "the agent bridge still feels awkward with the polling." The target is to make `wait_final` the first running-agent next action and keep `observe` as the diagnostic path.
