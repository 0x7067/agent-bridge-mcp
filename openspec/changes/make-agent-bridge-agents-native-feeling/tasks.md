## 1. API Shape

- [x] 1.1 Confirm the v1 API uses additive `task_*` responses with nested `presentation` metadata and no new `agent_*` tools.
- [x] 1.2 Implement the compact presentation summary shape from `design.md`, including display title, phase, status tone, timestamps, duration, workspace, result availability, change count, transcript availability, and result-evidence flags.
- [x] 1.3 Implement the action availability shape from `design.md`, including `available`, `unavailable`, and `unsafe` states with unavailable reasons.

## 2. Presentation Metadata

- [x] 2.1 Implement summary derivation from existing `TaskRecord` fields without duplicating task state.
- [x] 2.2 Implement action availability derivation for running, final, failed, stopped, stale, and managed-worktree tasks.
- [x] 2.3 Ensure compact presentation responses omit raw stdout, stderr, full diffs, and transcript event payloads.
- [x] 2.4 Keep final result and review packet inspection available through the existing detailed lifecycle surfaces.

## 3. Listing And Filtering

- [x] 3.1 Add a bounded default active/recent task presentation list with active tasks first, final tasks sorted by `updatedAt` descending, default limit 25, and max limit 100.
- [x] 3.2 Add optional `task_list` filters for scope, status, provider, mode, workspace/cwd, title text, and limit.
- [x] 3.3 Preserve an intentional path to inspect the full raw task registry for operational debugging.

## 4. Provider Capabilities

- [x] 4.1 Audit current runtime `providers_list` output for launch profiles, reduced-configuration metadata, reply support, resume support, and presentation-relevant action support; add only missing fields.
- [x] 4.2 Ensure runtime tool schemas expose launch profile arguments consistently with source and README examples.
- [x] 4.3 Add a runtime readiness snapshot that distinguishes static provider capabilities from provider states such as checking, ready, stale, and failed.
- [x] 4.4 Ensure version-only discovery never marks a provider as launchable unless startup readiness has been verified or explicitly caveated.
- [x] 4.5 Ensure `providers_check` or the selected readiness surface can refresh the runtime snapshot with checked timestamps, probe phase, timing fields, and diagnostics.
- [x] 4.6 Add production-binary fixture tests for `tools/list`, `providers_list`, and provider readiness drift.

## 5. Guidance And Documentation

- [x] 5.1 Update MCP prompts/resources to describe the native-client presentation workflow.
- [x] 5.2 Update README examples to show both native-presentation and raw lifecycle workflows.
- [x] 5.3 Document how clients should render unavailable reply and resume actions.
- [x] 5.4 Document that native-feeling completion still does not verify delegated work.

## 6. Verification

- [x] 6.1 Add unit tests for presentation summary derivation across queued/running/succeeded/failed/stopped/stale task states.
- [x] 6.2 Add unit tests for action availability, including unsupported reply/resume and managed-worktree cleanup guidance.
- [x] 6.3 Add integration tests for bounded active/recent listing and filters.
- [x] 6.4 Add stdio protocol tests for any new tools, arguments, or response fields.
- [x] 6.5 Add tests proving startup discovery is non-blocking and explicit rediscovery refreshes launchable-provider readiness.
- [x] 6.6 Run `cargo test`.
- [x] 6.7 Run `cargo fmt --check`.
- [x] 6.8 Run `cargo clippy --all-targets -- -D warnings`.
- [x] 6.9 Run `openspec validate make-agent-bridge-agents-native-feeling --strict`.

## 7. Dedicated Review

- [ ] 7.1 Run a named read-only Agent Bridge review task titled `Review native-feeling Agent Bridge presentation` using mode `review`; ask for `review-this`-inspired output with category ratings, an overall rating, a checklist summary, and actionable improvements, then inspect `task_result.reviewPacket` and raw evidence before deciding whether the change is ready to archive.
