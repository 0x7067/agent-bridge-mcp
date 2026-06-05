## 1. Tool Surface

- [x] 1.1 Add `agents_list` to `tools/list` with presentation-first filter schema and output schema.
- [x] 1.2 Add `agent_spawn` to `tools/list` with the same launch schema as `task_spawn`.
- [x] 1.3 Dispatch `agent_spawn` through the existing task manager spawn path.
- [x] 1.4 Dispatch `agents_list` through the existing task manager list path with presentation forced on and return `agents`.
- [x] 1.5 Mark `task_spawn` as legacy/deprecated in tool description without removing it.
- [x] 1.6 Reject `presentation` and `scope` as unknown `agents_list` arguments; use `task_list` for raw or full-history inspection.

## 2. Guidance

- [x] 2.1 Update initialization instructions to name `agent_spawn` and `agents_list`.
- [x] 2.2 Update prompts to prefer `agent_spawn`/`agents_list` and retain `task_*` lifecycle guidance.
- [x] 2.3 Update resources to describe the agent-oriented path and the deferred `task_spawn` removal.
- [x] 2.4 Update README tool lists, workflows, and examples so `agent_spawn`/`agents_list` are the preferred surface and `task_spawn` is legacy.

## 3. Specs And Tests

- [x] 3.1 Add OpenSpec deltas for agent launch/listing and guidance.
- [x] 3.2 Update `server_protocol.rs` exact tool list and guidance assertions.
- [x] 3.3 Update `stdio_binary.rs` tool count, schema, guidance, `agent_spawn`, `agents_list`, and legacy `task_spawn` assertions.
- [x] 3.4 Add tests that `agents_list` returns `agents` rather than `tasks`.
- [x] 3.5 Add tests that `agents_list` rejects `presentation` and `scope`.
- [x] 3.6 Add tests that `agent_spawn` returns the same lifecycle identifier shape as `task_spawn`.
- [x] 3.7 Run `cargo fmt --check`.
- [x] 3.8 Run focused Rust tests.
- [x] 3.9 Run `openspec validate add-agent-delegation-tools --strict`.
