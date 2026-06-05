## 1. Compatibility Research

- [x] 1.1 Write a compatibility memo summarizing current MCP task surfaces, including 2025-11-25 experimental tasks and newer task-extension migration constraints.
- [x] 1.2 Verify which task-related capabilities, methods, and metadata current target hosts can tolerate or advertise over stdio.
- [x] 1.3 Decide the first supported task surface, task-listing behavior, and target-host capability behavior, or explicitly mark implementation blocked until host/protocol compatibility is clearer.
- [x] 1.4 Update `design.md` and specs if the compatibility memo changes the selected protocol strategy.

## 2. Protocol Modeling

- [ ] 2.1 Add typed protocol models for the selected task capability and method shapes without advertising them yet.
- [ ] 2.2 Add mapping helpers from Agent Bridge task records to protocol task status, timestamps, status messages, TTL, and poll interval.
- [ ] 2.3 Add unit tests for queued/running/succeeded/failed/stopped/stale task-state mapping.
- [ ] 2.4 Add tests for unknown task ids and unsupported task methods.

## 3. Negotiated Capability Surface

- [ ] 3.1 Advertise task capabilities only when the selected compatibility surface is implemented.
- [ ] 3.2 Add initialize fixtures for clients with and without task support.
- [ ] 3.3 Ensure clients without task capabilities continue using existing `task_*` tools without behavior changes.

## 4. Task Operations

- [ ] 4.1 Implement the minimal supported task polling/status method from the compatibility design.
- [ ] 4.2 Implement protocol task cancellation by routing to existing stop/finalization behavior.
- [ ] 4.3 Ensure protocol cancellation does not remove logs, results, managed worktrees, or task registry records.
- [ ] 4.4 Add stdio tests for supported task operations, unsupported methods, unknown task ids, and terminal cancellation rejection.

## 5. Optional Notifications And Progress

- [x] 5.1 Decide whether status/progress notifications are in scope for the selected compatibility surface.
- [ ] 5.2 If in scope, emit only negotiated task status/progress notifications and keep polling sufficient for correctness.
- [ ] 5.3 Add tests proving notifications are optional and do not corrupt MCP stdout.

## 6. Guidance And Verification

- [x] 6.1 Update README and guidance resources to distinguish existing Agent Bridge task tools from protocol-level MCP task support.
- [x] 6.2 Run `cargo test`.
- [x] 6.3 Run `cargo fmt --check`.
- [x] 6.4 Run `cargo clippy --all-targets -- -D warnings`.
- [x] 6.5 Run `openspec validate explore-mcp-task-support --strict`.
