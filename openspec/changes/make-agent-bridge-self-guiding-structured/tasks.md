## 1. Initialization Guidance

- [x] 1.1 Add an Agent Bridge initialization instructions string with the safe workflow and verification boundary front-loaded.
- [x] 1.2 Return `instructions` from `initialize` while preserving the existing protocol version, server info, and capabilities.
- [x] 1.3 Add request-handler and stdio tests proving initialization instructions are present and existing initialize behavior remains compatible.

## 2. Structured Tool Results

- [x] 2.1 Update JSON tool result shaping to include `structuredContent` while preserving the existing serialized JSON text content.
- [x] 2.2 Add focused `outputSchema` metadata for `doctor`, `task_list`, `task_status`, `task_wait`, and `task_result` without over-constraining provider-specific diagnostics.
- [x] 2.3 Add tests proving structured content semantically matches the text JSON payload.
- [x] 2.4 Add tools-list fixture coverage for output schemas and existing strict input schemas.

## 3. Next-Action Metadata

- [x] 3.1 Define the `nextActions` array item shape with id, tool, arguments, state, reason, and safety classification.
- [x] 3.2 Derive ranked next actions for queued/running tasks, final uninspected tasks, final inspected tasks, managed-worktree cleanup, failed tasks, stopped tasks, and stale tasks.
- [x] 3.3 Expose `nextActions` metadata on task presentation and final result/review-packet surfaces.
- [x] 3.4 Add unit tests covering next-action derivation across lifecycle states and managed-worktree inspection state.
- [x] 3.5 Add stdio fixture coverage for running and final managed-worktree next actions.

## 4. Doctor Readiness Polish

- [x] 4.1 Add launch-readiness metadata that distinguishes setup health from provider startup verification.
- [x] 4.2 Add structured doctor recommendations with target tool names and minimal follow-up arguments where applicable.
- [x] 4.3 Add doctor tests for version-only providers that are available but not startup-verified or launchable.
- [x] 4.4 Add doctor tests proving recommendations do not expose secret values.

## 5. Guidance And Documentation

- [x] 5.1 Update MCP prompts and resources to mirror initialization instructions and mention structured results and next actions.
- [x] 5.2 Update README examples for the self-guiding structured workflow and doctor launch-readiness semantics.
- [x] 5.3 Document fallback behavior for clients that ignore instructions, structured content, output schemas, or next-action metadata.

## 6. Verification

- [x] 6.1 Run `cargo test`.
- [x] 6.2 Run `cargo fmt --check`.
- [x] 6.3 Run `cargo clippy --all-targets -- -D warnings`.
- [x] 6.4 Run `openspec validate make-agent-bridge-self-guiding-structured --strict`.
