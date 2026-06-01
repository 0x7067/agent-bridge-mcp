## 1. OpenSpec Housekeeping

- [x] 1.1 Verify completed active changes and archive `support-real-world-mcp-workflow`.
- [x] 1.2 Verify completed active changes and archive `improve-provider-smoke-reliability`.
- [x] 1.3 Replace archived spec `Purpose` placeholders with concrete purpose text where the archive process left `TBD`.

## 2. Review Packet

- [x] 2.1 Add failing stdio test coverage proving `task_result` includes `reviewPacket` for a successful no-change fake-provider task.
- [x] 2.2 Add failing stdio test coverage proving `reviewPacket.hasChanges`, `changedFiles`, and cleanup guidance for a managed-worktree task with repository changes.
- [x] 2.3 Add failing stdio test coverage proving failed task review packets include `errorType`, exit metadata, diagnostics, and recovery guidance.
- [x] 2.4 Add failing stdio test coverage proving running task review packets recommend bounded waits, incremental logs, status checks, or stopping.
- [x] 2.5 Implement review packet generation from existing task result fields.
- [x] 2.6 Keep `reviewPacket` additive and preserve existing `task_result` response fields.

## 3. Operator Guidance

- [x] 3.1 Add failing protocol/guidance tests for new host-runner lifecycle and dogfood workflow prompts/resources.
- [x] 3.2 Extend MCP prompt definitions and prompt text for host-runner lifecycle, provider comparison, and dogfood workflows.
- [x] 3.3 Extend MCP resource definitions and resource text for host-runner lifecycle and dogfood workflows.
- [x] 3.4 Update existing result-inspection guidance to mention `reviewPacket` as a summary alongside raw logs, diagnostics, git status, diff, and changed files.

## 4. Docs And Validation

- [x] 4.1 Update README examples to document `reviewPacket`, host-runner lifecycle, and dogfood workflows.
- [x] 4.2 Run `cargo fmt --check`.
- [x] 4.3 Run focused stdio/protocol tests.
- [x] 4.4 Run full `cargo test`.
- [x] 4.5 Run OpenSpec validation for `improve-operator-workflows`.
