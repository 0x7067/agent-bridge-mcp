## 1. Tool Surface Consolidation

- [ ] 1.1 Update `ToolName` (`src/tools.rs`) to the eight canonical tools and remove
  `ProvidersCheck`, `AgentPreview`, `AgentStatus`, `AgentWait`, `AgentLogs`,
  `AgentTranscript`.
- [ ] 1.2 Add `dryRun` (boolean) to the `agent_spawn` input schema; route `dryRun: true`
  through the existing preview path instead of spawning.
- [ ] 1.3 Add `until` (`"now" | "final"`, default `"now"`) to `agent_observe`; map
  `until: "final"` to the existing wait-to-finality path and `limit: 0` to a state-only read.
- [ ] 1.4 Add `sections` plus `maxBytes`/`stdoutLine`/`stderrLine`/`cursor` pagination to
  `agent_result`; default `sections: ["summary","changedFiles"]`.
- [ ] 1.5 Add `focus` (`"providers" | "all"`, default `"all"`) to `doctor`; route
  `focus: "providers"` through the existing readiness section only.
- [ ] 1.6 Update `call_tool` dispatch (`src/server.rs`) for the consolidated tools and
  remove dispatch arms for the deleted tools.
- [ ] 1.7 Add MCP `annotations` (`readOnlyHint`/`destructiveHint`/`idempotentHint`) to each
  tool definition.

## 2. Lean Response Envelope

- [ ] 2.1 Rewrite `observe_payload` (`src/task.rs`) to emit each field once: `agentId`,
  `status`, `isFinal`, `phase`, `progress`, `events`, `nextCursor`, `timedOut`, `next`.
- [ ] 2.2 Remove the GUI `presentation`/`presentation_actions` builders from agent-facing
  responses; introduce a single deduplicated `next` builder reused by observe/result/list.
- [ ] 2.3 Add opt-in `verbosity: "detailed"` that re-adds debug metadata (timestamps,
  `profile`, `promptStrategy`, diagnostics) to observe/result responses.
- [ ] 2.4 Slim `agent_list` per-agent records to lean fields plus a primary `next` action.
- [ ] 2.5 Update `agent_result`/`review_packet` so default responses are compact and large
  evidence is returned only when its `sections` are requested.
- [ ] 2.6 Update `outputSchema` definitions in `src/tools.rs` for the lean envelopes.

## 3. Guidance And Docs

- [ ] 3.1 Update initialization instructions and prompts/resources (`src/guidance.rs`) for
  the eight-tool surface; remove references to the removed tools.
- [ ] 3.2 Add the `agent-bridge://guidance/code-execution` resource describing compact
  polling, on-demand evidence sections, and caller-owned verification.
- [ ] 3.3 Update README tool list, recommended workflow, and add a migration table from the
  removed tools to their subsuming parameters.

## 4. Specs And Tests

- [ ] 4.1 Keep the OpenSpec deltas in this change in sync with the implementation.
- [ ] 4.2 Update `tests/server_protocol.rs` exact tool list, schema, and guidance assertions.
- [ ] 4.3 Update `tests/stdio_binary.rs` tool count/inventory, schemas, guidance, and
  next-action assertions; assert removed tools are absent.
- [ ] 4.4 Add tests proving `agent_observe` returns no duplicated `nextActions`/`progress`/
  `presentation` and that the lean envelope contains every field a caller needs.
- [ ] 4.5 Add tests for `agent_spawn dryRun`, `agent_observe until/limit:0`,
  `agent_result sections`+pagination, and `doctor focus: "providers"`.
- [ ] 4.6 Update `task.rs` presentation/next-action unit tests for the `next` list.

## 5. Validation

- [ ] 5.1 Run `cargo fmt --check`.
- [ ] 5.2 Run focused protocol and stdio tests, then full `cargo test` and
  `cargo clippy --all-targets -- -D warnings`.
- [ ] 5.3 Run `openspec validate optimize-agent-tool-ergonomics --strict`.
- [ ] 5.4 Build the release binary and smoke `initialize`, `tools/list`, `doctor`,
  `agent_spawn`, `agent_observe`, `agent_result` on the installed binary.
