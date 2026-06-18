## 1. Compatibility Research

- [x] 1.1 Write a compatibility memo summarizing current MCP task surfaces, including 2025-11-25 experimental tasks and newer task-extension migration constraints.
- [x] 1.2 Verify which task-related capabilities, methods, and metadata current target hosts can tolerate or advertise over stdio.
- [x] 1.3 Decide the first supported task surface, task-listing behavior, and target-host capability behavior, or explicitly mark implementation blocked until host/protocol compatibility is clearer.
- [x] 1.4 Update `design.md` and specs if the compatibility memo changes the selected protocol strategy.

## 2. Protocol Gating

- [x] 2.1 Keep `initialize` free of MCP task capability advertisement before a supported negotiated surface exists.
- [x] 2.2 Keep `tasks/get`, `tasks/update`, `tasks/cancel`, `tasks/list`, and `tasks/result` unsupported with JSON-RPC method-not-found errors.
- [x] 2.3 Keep protocol task methods out of `tools/list`.

## 3. Task Metadata Readiness

- [x] 3.1 Report `unavailable` readiness when clients send no task metadata.
- [x] 3.2 Report current `io.modelcontextprotocol/tasks` extension metadata without advertising tasks.
- [x] 3.3 Report legacy, unknown, and conflicting task metadata safely.
- [x] 3.4 Read request `_meta` task-extension metadata without leaking raw metadata values.

## 4. Native Lifecycle Preservation

- [x] 4.1 Keep the stable native lifecycle on `agent_spawn`, `agent_list`, `agent_observe`, `agent_result`, `agent_stop`, and `agent_remove`.
- [x] 4.2 Ensure task metadata does not create public `task_*` tools or `tasks/*` tool aliases.
- [x] 4.3 Keep caller guidance pointed at the native `agent_*` lifecycle while protocol tasks are unavailable.

## 5. Optional Notifications And Progress

- [x] 5.1 Decide whether status/progress notifications are in scope for the selected compatibility surface.
- [x] 5.2 Keep protocol task status/progress notifications out of scope while task support is unadvertised.
- [x] 5.3 Keep polling through existing `agent_observe`/`agent_result` sufficient for correctness.

## 6. Guidance And Verification

- [x] 6.1 Update README and guidance resources to distinguish existing Agent Bridge task tools from protocol-level MCP task support.
- [x] 6.2 Run `cargo test`.
- [x] 6.3 Run `cargo fmt --check`.
- [x] 6.4 Run `cargo clippy --all-targets -- -D warnings`.
- [x] 6.5 Run `openspec validate explore-mcp-task-support --strict`.
