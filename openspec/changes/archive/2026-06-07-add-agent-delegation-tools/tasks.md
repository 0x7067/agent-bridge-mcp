## 1. Tool Surface

- [x] 1.1 Rename the presentation list surface from `agents_list` to canonical `agent_list`.
- [x] 1.2 Keep `agent_spawn` as the canonical provider-agent launch tool.
- [x] 1.3 Add canonical lifecycle tools: `agent_status`, `agent_wait`, `agent_logs`, `agent_transcript`, `agent_result`, `agent_stop`, and `agent_remove`.
- [x] 1.4 Rename `task_preview` to canonical `agent_preview`.
- [x] 1.5 Remove the parallel public `task_*` tools from `tools/list` after the canonical path is covered by tests.
- [x] 1.6 Dispatch all canonical `agent_*` tools through the existing task manager paths without renaming persisted `taskId` values.
- [x] 1.7 Reject legacy-only list arguments on `agent_list`; raw/full-history registry inspection is no longer a separate advertised tool.
- [x] 1.8 Add `agent_observe` to the canonical tool inventory and ship this change together with progress observation.
- [x] 1.9 Update `ToolName` enum, `tools/call` dispatch, tool descriptions, and output schemas for canonical names rather than aliases.

## 2. Guidance

- [x] 2.1 Update initialization instructions to name the canonical `agent_*` tool family.
- [x] 2.2 Update prompts to prefer one agent-oriented lifecycle and remove `task_*` lifecycle guidance.
- [x] 2.3 Update resources to describe the agent-oriented path without a deferred `task_spawn` migration.
- [x] 2.4 Update README tool lists, workflows, and examples so the public workflow uses canonical `agent_*` names.
- [x] 2.5 Update doctor task-extension readiness copy so `recommendedNextStep` points to `agent_*` tools.
- [x] 2.6 Add a breaking-change migration table from old `task_*`/`agents_list` names to canonical `agent_*` names and state that `taskId` values are unchanged.

## 3. Specs And Tests

- [x] 3.1 Add OpenSpec deltas for agent launch/listing and guidance.
- [x] 3.2 Update `server_protocol.rs` exact tool list and guidance assertions.
- [x] 3.3 Update `stdio_binary.rs` tool count, schema, guidance, and canonical `agent_*` assertions.
- [x] 3.4 Add tests that `agent_list` returns `agents` rather than `tasks`.
- [x] 3.5 Add tests that `agent_list` rejects legacy raw-list arguments.
- [x] 3.6 Add tests that `agent_spawn` returns the persisted lifecycle identifier shape.
- [x] 3.7 Add tests proving legacy `task_*` tools are no longer advertised.
- [x] 3.8 Update presentation/nextActions tests so action tool names use `agent_*`.
- [x] 3.9 Update doctor readiness tests so recommendations use `agent_*`.
- [x] 3.10 Update/remove `task_list` raw-history tests that no longer apply to the public tool surface.
- [x] 3.11 Run `cargo fmt --check`.
- [x] 3.12 Run focused Rust tests.
- [x] 3.13 Run `openspec validate add-agent-delegation-tools --strict`.
