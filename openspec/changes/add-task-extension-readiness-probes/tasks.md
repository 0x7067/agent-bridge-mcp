## 1. Readiness Model

- [ ] 1.1 Define a task-extension readiness classification enum or equivalent structured values: `unavailable`, `extension_capable`, `legacy_only`, `unknown`, and `unsupported`.
- [ ] 1.2 Add metadata parsing helpers for current `io.modelcontextprotocol/tasks` extension declarations, legacy task metadata, and unknown task-like metadata.
- [ ] 1.3 Ensure readiness parsing is request-scoped and does not persist raw client metadata.

## 2. Diagnostic Surface

- [ ] 2.1 Choose the narrow diagnostic surface for readiness output, such as an additive diagnostic tool field or a focused readiness tool/resource.
- [ ] 2.2 Return `serverAdvertisesTasks: false`, observed extension identifiers, classification, and recommended next step.
- [ ] 2.3 Ensure no `tasks/*` method availability, `CreateTaskResult`, protocol task listing, or protocol cancellation is exposed.

## 3. Host Compatibility Fixtures

- [ ] 3.1 Add stdio fixture coverage for clients with no task-extension metadata.
- [ ] 3.2 Add stdio fixture coverage for current `io.modelcontextprotocol/tasks` extension metadata.
- [ ] 3.3 Add stdio fixture coverage for legacy 2025-11-25 task metadata.
- [ ] 3.4 Add stdio fixture coverage for unknown task-like metadata.
- [ ] 3.5 Add a regression test proving unsupported `tasks/*` methods still return existing method-not-found behavior.

## 4. Side-Effect Safety

- [ ] 4.1 Add tests proving readiness probes do not create task records, logs, transcripts, managed worktrees, or provider processes.
- [ ] 4.2 Add tests proving existing `task_*` lifecycle behavior remains the execution path after readiness probing.

## 5. Guidance

- [ ] 5.1 Update README and guidance resources to describe readiness probes as diagnostic evidence only.
- [ ] 5.2 Document that protocol-level `tasks/*`, `CreateTaskResult`, listing, cancellation, and notifications remain unavailable until a future implementation change.
- [ ] 5.3 Cross-link the readiness probe change to the existing MCP task compatibility memo.

## 6. Verification

- [ ] 6.1 Run `cargo test`.
- [ ] 6.2 Run `cargo fmt --check`.
- [ ] 6.3 Run `cargo clippy --all-targets -- -D warnings`.
- [ ] 6.4 Run `openspec validate add-task-extension-readiness-probes --strict`.
