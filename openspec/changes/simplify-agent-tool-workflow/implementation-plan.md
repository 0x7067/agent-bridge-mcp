## Implementation Plan

### Scope

Implement OpenSpec change `simplify-agent-tool-workflow`.

This is a non-breaking guidance and presentation simplification. It must preserve every current MCP tool name, input schema, response shape, and strict unknown-argument behavior.

### Files Expected To Change

- `crates/agent-bridge-mcp/src/guidance.rs`
- `crates/agent-bridge-mcp/src/tools.rs`
- `crates/agent-bridge-mcp/tests/server_protocol.rs`
- `crates/agent-bridge-mcp/tests/stdio_binary.rs`
- `README.md`
- `openspec/changes/simplify-agent-tool-workflow/tasks.md`

### Steps

1. Update initialization instructions in `guidance.rs` so the primary path is `doctor` when uncertain, optional focused readiness, `agent_spawn`, `agent_observe`, `agent_result`, caller verification, and intentional cleanup.
2. Rewrite `CALLER_WORKFLOW_RESOURCE` and prompt/resource text in `guidance.rs` into primary workflow and diagnostic/recovery sections so `agent_preview`, `agent_list`, `agent_status`, `agent_wait`, `agent_logs`, and `agent_transcript` are contextual diagnostics or presentation tools instead of required default steps.
3. Update README "What It Provides" and "Recommended Workflow" to group primary lifecycle tools separately from diagnostic/advanced tools, removing `agent_preview` and multi-tool monitoring from the default numbered path.
4. Update `tools.rs` descriptions without changing tool names, schemas, required fields, enum values, or dispatch behavior.
5. Update protocol tests for initialization wording, prompt/resource wording, tool descriptions, and current tool names with no `task_*` lifecycle entries.
6. Add schema-compatibility assertions for all current public tools so description edits cannot accidentally change input schemas.
7. Update stdio tests for public tool list compatibility and confirm next-action ordering remains regression-covered without runtime changes.
7. Run formatting, focused tests, full tests, clippy, OpenSpec validation, release build, installed-binary refresh, binary comparison, and installed-binary smoke.

### Risks

- Wording-only tests can become brittle. Prefer key phrase and ordering assertions over exact paragraph matching.
- Tool description edits could accidentally imply behavior changes. Keep descriptions accurate and avoid promising automatic verification.
- Existing docs intentionally mention raw logs/transcripts for failure recovery. Preserve those sections while de-emphasizing them for successful default delegation.
- Tests should prove ordering or section boundaries for the compact path instead of only checking that tool names appear somewhere.

### Review Questions

- Does this plan preserve enough raw-inspection guidance for real failure cases?
- Are there additional tests needed to prove no public MCP schema changed?
- Is the README grouping clear without implying advanced tools are deprecated?
