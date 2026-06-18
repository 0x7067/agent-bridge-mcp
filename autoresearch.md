# Autoresearch: agent bridge cost efficiency

## Objective
Reduce Agent Bridge caller-facing cost for common MCP discovery and guidance flows.
The workload measures actual JSON-RPC response bytes from the stdio server for:
`initialize`, `tools/list`, `prompts/list`, every prompt body, `resources/list`,
every guidance resource, `providers_list`, and one `agent_spawn` dry run.

## Metrics
- **Primary**: `total_bytes` (serialized JSON response bytes, lower is better)
- **Secondary**: `initialize_bytes`, `tools_list_bytes`, `prompts_bytes`,
  `resources_bytes`, `providers_list_bytes`, `dryrun_bytes`

## How to Run
`./autoresearch.sh` - outputs `METRIC name=number` lines.

## Files in Scope
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
- Lower bytes only count when protocol tests still pass.
- Prefer deletion and concise wording over abstractions.
- Keep strict schemas and compatibility assertions.
- Discard any reduction that makes guidance ambiguous about verification, raw evidence access, or cleanup safety.

## What's Been Tried
- Run 1 baseline: 54498 bytes. Largest measured buckets are provider capability JSON (17252), resources (15126), prompts (9982), and tools/list (8854).
- Run 2 kept: shortened initialization guidance to 54231 bytes without losing required safety/workflow markers.
