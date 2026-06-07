## Why

Agent Bridge now has a consistent `agent_*` public lifecycle, but the advertised MCP workflow still presents most lifecycle tools as equally primary. That makes the surface feel larger than the common delegation path requires and encourages callers to choose among low-level inspection tools before they need them.

## What Changes

- Define a smaller primary workflow in initialization instructions, prompts, resources, and README:
  `doctor` when uncertain, optional focused readiness checks, `agent_spawn`, `agent_observe`, `agent_result`, caller-owned verification, and intentional cleanup.
- Reframe `providers_check`, `agent_preview`, `agent_list`, `agent_status`, `agent_wait`, `agent_logs`, and `agent_transcript` as focused readiness, presentation, or diagnostic escape hatches instead of default steps.
- Keep all existing MCP tool names, inputs, output fields, and strict validation behavior callable.
- Keep `nextActions` machine-actionable while ranking `agent_observe` as the primary running-agent action and `agent_result` as the primary final-agent action.
- Add focused tests that guard the simplified guidance and ensure no legacy `task_*` lifecycle tools are reintroduced.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `agent-bridge-self-guidance`: Initialization instructions and next-action requirements should describe the smaller primary workflow and treat lower-level tools as diagnostics.
- `mcp-usage-guidance`: Prompt and resource guidance should teach the smaller primary workflow while preserving a documented fallback for manual inspection.
- `agent-bridge-agent-presentation`: Presentation action guidance should keep raw inspection tools available without making them the primary path.
- `rust-single-binary-mcp`: Public MCP tool-surface expectations should preserve all current tools while testing simplified descriptions and absence of duplicate legacy lifecycle tools.

## Impact

- Affected code: `crates/agent-bridge-mcp/src/guidance.rs`, `crates/agent-bridge-mcp/src/tools.rs`, task next-action tests if wording changes.
- Affected docs: `README.md`.
- Affected tests: protocol and stdio fixtures covering initialization instructions, guidance resources/prompts, tool descriptions, and public tool names.
- No dependency, provider adapter, registry, transport, or protocol-level MCP Tasks changes.
- No breaking public API changes.
