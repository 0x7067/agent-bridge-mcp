# Optimize agent tool ergonomics, efficiency, and discoverability

## Why

Agent Bridge already has clean `agent_*` namespacing, enum-constrained inputs,
`nextActions` guidance, and self-describing prompts/resources. But two ergonomics problems
remain when the consumer is an LLM agent (the only consumer of these payloads), measured
against current agent-tooling best practice (Anthropic's *Writing effective tools for
agents*, *Code execution with MCP*, and *Advanced tool use*):

1. **The primary polled tool emits massively duplicated payloads.** A single
   `agent_observe` response (`observe_payload`, `crates/agent-bridge-mcp/src/task.rs`)
   serializes `nextActions` four times (top level, inside `agent`, inside
   `agent.presentation`, inside `presentation`), `progress` three times, and the
   GUI-oriented `presentation` blob (`displayTitle`, `subtitle`, `statusTone`, timestamps,
   a ten-entry UI `actions` array) twice. Because `agent_observe` runs in a polling loop,
   this duplication compounds on every poll and burns context for no agent benefit.

2. **Fourteen tools, six of which overlap on "read one agent's state."**
   `agent_status`, `agent_wait`, `agent_logs`, `agent_transcript`, `agent_observe`, and
   `agent_result` all read a single agent; `agent_preview` duplicates `agent_spawn`; and
   `providers_check` is already a strict subset of `doctor`. Loading all fourteen upfront
   costs context, and selection accuracy degrades as redundant tools accumulate.

The earlier `simplify-agent-tool-workflow` change reframed these tools as "diagnostic
escape hatches" in guidance but deliberately kept every tool and payload. This change is
its successor: it actually reduces the surface and slims the payloads instead of only
re-ranking them in prose.

## What Changes

- **BREAKING** Consolidate the public tool surface from fourteen tools to eight by folding
  redundant reads into their primary tool via parameters:
  - `agent_observe` subsumes `agent_status` (state-only read) and `agent_wait` (block to
    finality) via an `until` mode and `limit: 0`; its `events` already are the transcript,
    subsuming `agent_transcript`.
  - `agent_result` subsumes `agent_logs` via a `sections` selector with pagination.
  - `agent_spawn` subsumes `agent_preview` via `dryRun: true`.
  - `doctor` subsumes `providers_check` via a `focus: "providers" | "all"` selector
    (doctor already runs the identical readiness engine).
  - Removed public tools: `providers_check`, `agent_preview`, `agent_status`,
    `agent_wait`, `agent_logs`, `agent_transcript`.
- **BREAKING** Replace the duplicated, GUI-oriented agent response envelope with a single
  lean agent-facing envelope. `agent_observe` returns each field once
  (`agentId`, `status`, `isFinal`, `phase`, `progress`, `events`, `nextCursor`,
  `timedOut`, `next`); the `agent` full-record echo and both `presentation` copies are
  removed. A single deduplicated `next` array replaces the four `nextActions`/`presentation`
  action copies. An opt-in `verbosity: "detailed"` re-adds debug metadata.
- Make the API code-execution / Tool-Search friendly: large evidence
  (full `stdout`/`stderr`/`diff`/`transcript`) is fetched on demand via `agent_result`
  sections and pagination rather than dumped by default, keeping intermediate data out of
  the model context; tools carry MCP `annotations` (`readOnlyHint`, `destructiveHint`,
  `idempotentHint`) so Tool-Search-capable clients can tier and defer them; and a new
  `agent-bridge://guidance/code-execution` resource documents the compact polling and
  on-demand evidence pattern.
- Update initialization instructions, prompts, resources, README, and the migration table
  for the eight-tool surface.

## Non-Goals

- No change to the internal task registry, persisted `agentId`/`taskId`, provider adapters,
  the Claude host runner, or the readiness/smoke engine semantics.
- No protocol-level MCP Tasks and no server-pushed JSON-RPC notifications.
- No GUI/native presentation contract is preserved: agents are the only consumers, so the
  GUI `presentation`/`actions` payload is removed rather than gated.

## Capabilities

### New Capabilities
- None. (A new code-execution guidance requirement is added to `mcp-usage-guidance`.)

### Modified Capabilities
- `rust-single-binary-mcp`: public tool inventory is eight consolidated tools; subsuming
  parameters (`until`, `sections`, `dryRun`, `focus`) replace removed tools.
- `agent-bridge-agent-presentation`: agent reads return one lean envelope; the GUI
  structured-action-availability contract is removed.
- `agent-bridge-self-guidance`: ranked next actions are emitted once as a deduplicated
  `next` list aimed at the eight-tool surface.
- `delegated-review-packet`: `agent_result` gains a `sections` selector and pagination,
  defaults to a compact packet, and subsumes raw log inspection.
- `task-run-transcripts`: transcript events are inspected through `agent_observe` rather
  than a separate `agent_transcript` tool.
- `agent-bridge-doctor`: `doctor` exposes a `focus` selector and is the single readiness
  entry point.
- `provider-readiness-contract`: readiness controls are advertised on `doctor` instead of
  `providers_check`.
- `mcp-usage-guidance`: prompts/resources teach the eight-tool workflow and a
  code-execution-friendly delegation pattern.

## Impact

- Affected code: `crates/agent-bridge-mcp/src/tools.rs` (`ToolName`, definitions, schemas,
  annotations), `src/server.rs` (`call_tool` dispatch), `src/task.rs`
  (`observe_payload`/`public_task`/`presentation`/`next_actions`/`review_packet`),
  `src/guidance.rs` (instructions, prompts, resources), `src/provider.rs`.
- Affected docs: `README.md` tool list, recommended workflow, and migration table.
- Affected tests: `tests/server_protocol.rs`, `tests/stdio_binary.rs` (tool count/inventory,
  schemas, guidance, next-action assertions) and `task.rs` presentation/next-action units.
- Compatibility: intentional public API break consistent with the repo's prior
  `agent_*`/`agentId` simplification. Callers replace removed tools with the documented
  subsuming parameters; persisted `agentId`/`taskId` values are unchanged.
