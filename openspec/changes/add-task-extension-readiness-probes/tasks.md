## 1. Readiness Model

- [x] 1.1 Define a task-extension readiness classification enum or equivalent structured values: `unavailable`, `extension_capable`, `legacy_only`, `unknown`, and `unsupported`.
- [x] 1.2 Add metadata parsing helpers for current `io.modelcontextprotocol/tasks` extension declarations, legacy task metadata, and unknown task-like metadata.
- [x] 1.3 Ensure readiness parsing is request-scoped and does not persist raw client metadata.
- [x] 1.4 Add a process-lifetime derived readiness snapshot updated from initialize params and request metadata, storing no raw metadata.

## 2. Diagnostic Surface

- [x] 2.1 Add `doctor.taskExtensionReadiness` as the narrow diagnostic surface.
- [x] 2.2 Return `serverAdvertisesTasks: false`, observed extension identifiers, classification, and recommended next step.
- [x] 2.3 Ensure no `tasks/*` method availability, `CreateTaskResult`, protocol task listing, or protocol cancellation is exposed.
- [x] 2.4 Preserve `summary.status` aggregation and existing doctor sections.

## 3. Host Compatibility Fixtures

- [x] 3.1 Add stdio fixture coverage for clients with no task-extension metadata.
- [x] 3.2 Add stdio fixture coverage for current `io.modelcontextprotocol/tasks` extension metadata.
- [x] 3.3 Add stdio fixture coverage for legacy 2025-11-25 task metadata.
- [x] 3.4 Add stdio fixture coverage for unknown task-like metadata.
- [x] 3.5 Add a regression test proving unsupported `tasks/*` methods still return existing method-not-found behavior.
- [x] 3.6 Add stdio harness support for parameterized initialize metadata.

## 4. Side-Effect Safety

- [x] 4.1 Add tests proving readiness probes do not create task records, logs, transcripts, managed worktrees, or provider processes.
- [x] 4.2 Add tests proving existing `task_*` lifecycle behavior remains the execution path after readiness probing.
- [x] 4.3 Add tests proving raw client metadata is not written to state files or public doctor responses.

## 5. Guidance

- [x] 5.1 Update README and guidance resources to describe readiness probes as diagnostic evidence only.
- [x] 5.2 Document that protocol-level `tasks/*`, `CreateTaskResult`, listing, cancellation, and notifications remain unavailable until a future implementation change.
- [x] 5.3 Cross-link the readiness probe change to the existing MCP task compatibility memo.

## 6. Verification

- [x] 6.1 Run `cargo test`.
- [x] 6.2 Run `cargo fmt --check`.
- [x] 6.3 Run `cargo clippy --all-targets -- -D warnings`.
- [x] 6.4 Run `openspec validate add-task-extension-readiness-probes --strict`.
