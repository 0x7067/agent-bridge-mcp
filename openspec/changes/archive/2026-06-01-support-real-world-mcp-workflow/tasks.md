## 1. MCP Host Compatibility

- [x] 1.1 Add a failing production-binary stdio test that sends `tools/call` params with `name`, `arguments`, and envelope `_meta`.
- [x] 1.2 Update tool-call envelope parsing with an explicit `_meta` field while continuing to reject any other unknown envelope field.
- [x] 1.3 Add regression coverage proving envelope `_meta` is not forwarded into the selected tool's public argument object.
- [x] 1.4 Add regression coverage proving unknown fields inside `arguments` still return unknown-argument tool errors.
- [x] 1.5 Confirm `tools/list` schemas still advertise `additionalProperties: false` for public tool argument objects.

## 2. Delegation Workflow Harness

- [x] 2.1 Replace `AGENT_BRIDGE_ALLOWED_ROOT` with `AGENT_BRIDGE_WORKSPACES` in workspace confinement code and provider environment allowlists.
- [x] 2.2 Add tests for multiple configured workspace roots and outside-workspace rejection.
- [x] 2.3 Add tests proving `AGENT_BRIDGE_ALLOWED_ROOT` is no longer used.
- [x] 2.4 Document the standard MCP caller workflow from provider readiness through task cleanup.
- [x] 2.5 Document stalled-task handling with bounded `task_wait`, incremental `task_logs`, `task_stop`, and final `task_result` inspection.
- [x] 2.6 Document that write-capable delegated tasks should use managed worktree isolation by default.
- [x] 2.7 Document that provider task results are evidence and do not replace main-thread verification gates.

## 3. Live Smoke Workflow

- [x] 3.1 Add an opt-in local live-smoke workflow for `providers_check` with `smoke: true` and bounded timeout guidance.
- [x] 3.2 Add guidance for a minimal read-only live task smoke that avoids workspace mutation by default.
- [x] 3.3 Keep live provider smoke checks out of default CI and document any required local credentials or provider CLIs.

## 4. Verification

- [x] 4.1 Run `cargo fmt --check`.
- [x] 4.2 Run `cargo test`.
- [x] 4.3 Run `cargo clippy --all-targets -- -D warnings`.
- [x] 4.4 Run `openspec validate support-real-world-mcp-workflow`.
