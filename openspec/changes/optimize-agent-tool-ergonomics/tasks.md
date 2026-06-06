## 1. Tool Surface Consolidation

- [x] 1.1 Update `ToolName` (`src/tools.rs`) to the eight canonical tools and remove
  `ProvidersCheck`, `AgentPreview`, `AgentStatus`, `AgentWait`, `AgentLogs`,
  `AgentTranscript`.
- [x] 1.2 Add `dryRun` (boolean) to the `agent_spawn` input schema; route `dryRun: true`
  through the existing preview path instead of spawning.
- [x] 1.3 Add `until` (`"now" | "final"`, default `"now"`) to `agent_observe`; map
  `until: "final"` to the existing wait-to-finality path and `limit: 0` to a state-only read.
- [x] 1.4 Add `sections` plus `maxBytes`/`stdoutLine`/`stderrLine`/`cursor` pagination to
  `agent_result`; default `sections: ["summary","changedFiles"]`.
- [x] 1.5 Add `focus` (`"providers" | "all"`, default `"all"`) to `doctor`; route
  `focus: "providers"` through the existing readiness section only. (Also threaded the
  per-provider `timeoutMs` budget through `doctor` so it fully replaces `providers_check`.)
- [x] 1.6 Update `call_tool` dispatch (`src/server.rs`) for the consolidated tools and
  remove dispatch arms for the deleted tools.
- [x] 1.7 Add MCP `annotations` (`readOnlyHint`/`destructiveHint`/`idempotentHint`) to each
  tool definition.

## 2. Lean Response Envelope

- [x] 2.1 Rewrite `observe_payload` (`src/task.rs`) to emit each field once: `agentId`,
  `status`, `isFinal`, `phase`, `progress`, `events`, `nextCursor`, `timedOut`, `next`.
- [x] 2.2 Remove the GUI `presentation`/`presentation_actions` builders from agent-facing
  responses; introduce a single deduplicated `next` builder reused by observe/result/list.
- [x] 2.3 Add opt-in `verbosity: "detailed"` that re-adds debug metadata (timestamps,
  `profile`, `promptStrategy`, diagnostics) to observe/result responses. (`agent_spawn`
  returns detail by default since it is a one-shot launch.)
- [x] 2.4 Slim `agent_list` per-agent records to lean fields plus a primary `next` action.
- [x] 2.5 Update `agent_result`/`review_packet` so default responses are compact and large
  evidence is returned only when its `sections` are requested.
- [x] 2.6 Update `outputSchema` definitions in `src/tools.rs` for the lean envelopes.

## 3. Guidance And Docs

- [x] 3.1 Update initialization instructions and prompts/resources (`src/guidance.rs`) for
  the eight-tool surface; remove references to the removed tools.
- [x] 3.2 Add the `agent-bridge://guidance/code-execution` resource describing compact
  polling, on-demand evidence sections, and caller-owned verification.
- [x] 3.3 Update README tool list, recommended workflow, and add a migration table from the
  removed tools to their subsuming parameters.

## 4. Specs And Tests

- [x] 4.1 Keep the OpenSpec deltas in this change in sync with the implementation.
- [x] 4.2 Update `tests/server_protocol.rs` exact tool list, schema, and guidance assertions.
- [x] 4.3 Update `tests/stdio_binary.rs` tool count/inventory, schemas, guidance, and
  next-action assertions; assert removed tools are absent.
- [x] 4.4 Add tests proving `agent_observe` returns no duplicated `nextActions`/`progress`/
  `presentation` and that the lean envelope contains every field a caller needs.
- [x] 4.5 Add tests for `agent_spawn dryRun`, `agent_observe until/limit:0`,
  `agent_result sections`+pagination, and `doctor focus: "providers"`.
- [x] 4.6 Update `task.rs` presentation/next-action unit tests for the `next` list.

## 5. Validation

- [x] 5.1 Run `cargo fmt --check`.
- [x] 5.2 Run focused protocol and stdio tests, then full `cargo test` and
  `cargo clippy --all-targets -- -D warnings`. (clippy clean across all targets; full suite
  green except two pre-existing, environment-specific process-group reaping tests —
  `stdio_codex_agent_sandbox_denial_hangs_and_is_terminated_early` and
  `stdio_providers_check_timeout_fallback_and_process_group_cleanup` — which fail
  identically on the base commit in this sandbox, unrelated to this change.)
- [ ] 5.3 Run `openspec validate optimize-agent-tool-ergonomics --strict`. (OpenSpec CLI is
  not installable in the execution sandbox; run on a maintainer machine.)
- [ ] 5.4 Build the release binary and smoke `initialize`, `tools/list`, `doctor`,
  `agent_spawn`, `agent_observe`, `agent_result` on the installed binary. (Run on a
  maintainer machine with provider CLIs available.)
