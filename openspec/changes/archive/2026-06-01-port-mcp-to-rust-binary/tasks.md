## 1. Compatibility Fixtures

- [x] 1.1 Create Rust stdio tests that run the JSON-RPC fixture set against the production binary.
- [x] 1.2 Define fixture normalization rules for task IDs, timestamps, durations, process IDs, map ordering, environment ordering, and pretty-printed tool payloads.
- [x] 1.3 Add golden fixtures for `initialize`, `notifications/initialized`, unknown notifications, stdin EOF shutdown, `tools/list`, `providers_list`, `providers_check`, validation failures, and `task_preview`.
- [x] 1.4 Add fixture coverage for full `tools/list` schemas, including `additionalProperties: false`, required fields, provider enums, and mode enums.
- [x] 1.5 Add fixture coverage for task lifecycle behavior with fake provider binaries: spawn, wait, logs, result, stop, timeout, stale startup recovery, active-task remove rejection, and remove failure.
- [x] 1.6 Add fixture coverage for safety invariants: cwd realpath confinement, symlink escape rejection, prompt byte cap, timeout clamps, wait timeout cap, `worktreeName` validation, and preview prompt redaction.
- [x] 1.7 Add fixture coverage for provider command descriptors and environment keys for Claude, Cursor, Kimi, and Codex.
- [x] 1.8 Keep compatibility coverage passing before substantial Rust behavior is implemented, then migrate ongoing coverage to Rust-only tests.

## 2. Rust Project Skeleton

- [x] 2.1 Add a single Rust binary crate for `agent-bridge-mcp`.
- [x] 2.2 Add typed MCP JSON-RPC request, notification, response, tool, and content DTOs for the current protocol surface.
- [x] 2.3 Add typed domain models for providers, modes, task states, phases, error types, isolation, prompts, safe cwd, timeouts, and worktree names.
- [x] 2.4 Select `tokio` or document a replacement async runtime before implementing stdio, process, or task lifecycle behavior.
- [x] 2.5 Mandate the non-blocking task-manager actor model and explicit actor panic abort mechanism before implementing task lifecycle behavior.
- [x] 2.6 Decide first release targets before storage and process work; default to macOS arm64, macOS x64, and Linux x64 unless explicitly changed.
- [x] 2.7 Preserve current unbounded task concurrency during the port and defer any maximum concurrent task limit to a later safety change.
- [x] 2.8 Decide whether to use the official Rust MCP SDK or local DTOs by running the compatibility fixtures against a minimal Rust skeleton and confirming expected fixture failures before green implementation.
- [x] 2.9 Add `cargo test`, formatting, and lint commands to the documented verification workflow.

## 3. Rust MCP Protocol And Tools

- [x] 3.1 Implement stdio newline-delimited JSON-RPC handling with no non-MCP stdout output.
- [x] 3.2 Configure stderr-only logging/tracing and a panic hook that writes diagnostics to stderr before protocol dispatch is implemented.
- [x] 3.3 Add a test or fixture assertion that protocol stdout contains only MCP JSON-RPC messages.
- [x] 3.4 Add a Rust integration test or fixture path that forces a panic and verifies diagnostics go to stderr, MCP stdout is not corrupted, and the process exits non-zero.
- [x] 3.5 Implement `initialize`, initialized notification handling, `tools/list`, and `tools/call` dispatch.
- [x] 3.6 Preserve current notification behavior: handle initialized notifications, ignore unknown notifications without response, and do not add `notifications/exit` behavior unless a fixture and compatibility rationale are added first.
- [x] 3.7 Implement stdin EOF handling so the runtime shuts down cleanly when the client disconnects.
- [x] 3.8 Implement strict typed tool input parsing with unknown-field rejection.
- [x] 3.9 Preserve JSON-RPC error behavior for invalid request, parse error, method not found, and internal error cases.
- [x] 3.10 Preserve tool-level `isError: true` behavior for validation and task errors.
- [x] 3.11 Implement `SIGINT` and `SIGTERM` handling that terminates tracked active provider children before exiting on supported Unix targets.
- [x] 3.12 Add a fixture or integration assertion that Rust sends `SIGTERM`, not Rust's default SIGKILL path, when stopping providers or handling server shutdown on supported Unix targets.

## 4. Rust Provider Adapters

- [x] 4.1 Implement fixed typed provider adapters for Claude, Cursor, Kimi, and Codex.
- [x] 4.2 Preserve provider capability metadata returned by `providers_list`.
- [x] 4.3 Preserve provider-specific mode and option validation for `effort` and `thinking`.
- [x] 4.4 Preserve provider command construction, prompt rendering, cwd, timeout, model, and provider-specific args.
- [x] 4.5 Preserve provider environment allowlists, including Claude `ANTHROPIC_BASE_URL` stripping.
- [x] 4.6 Preserve provider version command and startup smoke command behavior.
- [x] 4.7 Implement the public `providers_check` tool dispatch using adapter-owned version and smoke commands.

## 5. Rust Task Lifecycle

- [x] 5.1 Implement registry load/save with atomic writes on supported targets and compatibility for existing task records.
- [x] 5.2 Keep Rust registry writes inspectable and gate any versioned migration behind an explicit migration flag.
- [x] 5.3 Add Rust coverage that writes state with the binary and verifies tasks remain inspectable.
- [x] 5.4 Preserve the on-disk task layout `stateDir/tasks/<taskId>/stdout.log`, `stderr.log`, and `result.json` so rollback and inspection remain compatible.
- [x] 5.5 Serialize registry writes through the actor or a single writer queue so concurrent task completions cannot overlap atomic writes.
- [x] 5.6 Make persisted registry deserialization tolerant of unknown fields while keeping public tool inputs strict.
- [x] 5.7 Keep task IDs in the existing `task_` plus UUID-hex shape and retry if a generated ID collides with persisted state.
- [x] 5.8 Clean up or ignore known temporary registry files from crashed atomic writes before loading registry state.
- [x] 5.9 Keep atomic registry temp files in the same directory as canonical `registry.json` and add a fixture or unit test for same-directory temp paths.
- [x] 5.10 Fail startup with a clear diagnostic for a corrupted canonical `registry.json` and add a fixture test for that edge case.
- [x] 5.11 Implement the task-manager actor with bounded command and completion channels so task registry and active task access are serialized with explicit backpressure.
- [x] 5.12 Ensure the actor never awaits provider processes, git commands, log drains, or worktree cleanup directly; background tasks must send completion messages back to the actor without silently dropping final lifecycle updates.
- [x] 5.13 Ensure actor panic fails the server fast by monitoring the actor `JoinHandle` and aborting the process, or by using an equivalent process-wide abort strategy, instead of leaving request handlers waiting indefinitely.
- [x] 5.14 Add explicit state transition functions for queued, running, succeeded, failed, stopped, failed_stale, and removed.
- [x] 5.15 Implement startup stale recovery for previously queued or running tasks.
- [x] 5.16 Implement spawn, preview, list, status, wait, logs, result, stop, and remove behavior.
- [x] 5.17 Preserve current `task_remove` behavior by rejecting queued or running tasks until callers stop them or they reach a final state.
- [x] 5.18 Implement stdout/stderr capped logs while continuing to drain provider pipes after the cap.
- [x] 5.19 Decode provider stdout/stderr lossy for invalid UTF-8.
- [x] 5.20 Implement timeout, stop, provider start error, provider exit error, and stale error type recording.
- [x] 5.21 Ensure timeout, stop, and signal-shutdown paths await provider child exit after termination so Unix child processes are reaped.
- [x] 5.22 Enforce a bounded shutdown cleanup grace period before escalating unresponsive provider children and continuing server shutdown.
- [x] 5.23 Implement git status, git diff, changed files, worktree creation, and managed worktree cleanup.

## 6. Packaging And Entrypoint

- [x] 6.1 Define supported release targets and document provider CLI limitations per platform.
- [x] 6.2 Add a built binary smoke test for `initialize`, `tools/list`, `providers_list`, `providers_check`, and `task_preview`.
- [x] 6.3 Audit public tool names, input schemas, defaults, response shapes, state paths, and install commands; document either "no break" or exact migration notes.
- [x] 6.4 Implement the first install path as direct prebuilt binary releases for supported targets.
- [x] 6.5 During transition, expose the Rust binary under a distinct command such as `agent-bridge-mcp-rs`.
- [x] 6.6 After parity is proven, switch the final `agent-bridge-mcp` MCP entrypoint to the Rust binary.
- [x] 6.7 Use direct Rust binary releases as the project distribution path.
- [x] 6.8 Update README and example MCP config for the final binary entrypoint.

## 7. Verification

- [x] 7.1 Remove the former runtime/tests after Rust parity is proven.
- [x] 7.2 Run `cargo test` for Rust unit and integration tests.
- [x] 7.3 Run Rust stdio tests and confirm public response behavior.
- [x] 7.4 Run fake-provider lifecycle tests against the Rust binary.
- [x] 7.5 Run built or installed binary smoke tests.
- [x] 7.6 Run `cargo clippy` or an equivalent Rust lint gate that rejects stringly `serde_json::Value` use on tool dispatch hot paths unless explicitly justified.
- [x] 7.7 Run `openspec validate port-mcp-to-rust-binary` and confirm the change remains valid.
