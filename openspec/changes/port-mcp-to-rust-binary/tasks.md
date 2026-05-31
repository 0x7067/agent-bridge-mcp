## 1. Compatibility Fixtures

- [ ] 1.1 Create a stdio fixture harness that can run the same JSON-RPC fixture set against `node src/server.mjs` and a configurable binary command.
- [ ] 1.2 Define fixture normalization rules for task IDs, timestamps, durations, process IDs, map ordering, environment ordering, and pretty-printed tool payloads.
- [ ] 1.3 Add golden fixtures for `initialize`, `notifications/initialized`, unknown notifications, stdin EOF shutdown, `tools/list`, `providers_list`, `providers_check`, validation failures, and `task_preview`.
- [ ] 1.4 Add fixture coverage for full `tools/list` schemas, including `additionalProperties: false`, required fields, provider enums, and mode enums.
- [ ] 1.5 Add fixture coverage for task lifecycle behavior with fake provider binaries: spawn, wait, logs, result, stop, timeout, stale startup recovery, active-task remove rejection, and remove failure.
- [ ] 1.6 Add fixture coverage for safety invariants: cwd realpath confinement, symlink escape rejection, prompt byte cap, timeout clamps, wait timeout cap, `worktreeName` validation, and preview prompt redaction.
- [ ] 1.7 Add fixture coverage for provider command descriptors and environment keys for Claude, Cursor, Kimi, and Codex.
- [ ] 1.8 Keep the Node fixture run passing and make it the compatibility baseline before substantial Rust behavior is implemented.

## 2. Rust Project Skeleton

- [ ] 2.1 Add a single Rust binary crate for `agent-bridge-mcp` without replacing the existing Node entrypoint.
- [ ] 2.2 Add typed MCP JSON-RPC request, notification, response, tool, and content DTOs for the current protocol surface.
- [ ] 2.3 Add typed domain models for providers, modes, task states, phases, error types, isolation, prompts, safe cwd, timeouts, and worktree names.
- [ ] 2.4 Select `tokio` or document a replacement async runtime before implementing stdio, process, or task lifecycle behavior.
- [ ] 2.5 Mandate the non-blocking task-manager actor model and actor panic policy before implementing task lifecycle behavior.
- [ ] 2.6 Decide first release targets before storage and process work; default to macOS arm64, macOS x64, and Linux x64 unless explicitly changed.
- [ ] 2.7 Decide whether to preserve unbounded task concurrency or add a maximum concurrent task limit during the port.
- [ ] 2.8 Decide whether to use the official Rust MCP SDK or local DTOs by running the compatibility fixtures against a minimal Rust skeleton and confirming expected fixture failures before green implementation.
- [ ] 2.9 Add `cargo test`, formatting, and lint commands to the documented verification workflow.

## 3. Rust MCP Protocol And Tools

- [ ] 3.1 Implement stdio newline-delimited JSON-RPC handling with no non-MCP stdout output.
- [ ] 3.2 Configure stderr-only logging/tracing and a panic hook that writes diagnostics to stderr before protocol dispatch is implemented.
- [ ] 3.3 Add a test or fixture assertion that protocol stdout contains only MCP JSON-RPC messages.
- [ ] 3.4 Add a Rust integration test or fixture path that forces a panic and verifies diagnostics go to stderr without corrupting MCP stdout.
- [ ] 3.5 Implement `initialize`, initialized notification handling, `tools/list`, and `tools/call` dispatch.
- [ ] 3.6 Preserve current notification behavior: handle initialized notifications, ignore unknown notifications without response, and do not add `notifications/exit` behavior unless a fixture and compatibility rationale are added first.
- [ ] 3.7 Implement stdin EOF handling so the runtime shuts down cleanly when the client disconnects.
- [ ] 3.8 Implement strict typed tool input parsing with unknown-field rejection.
- [ ] 3.9 Preserve JSON-RPC error behavior for invalid request, parse error, method not found, and internal error cases.
- [ ] 3.10 Preserve tool-level `isError: true` behavior for validation and task errors.
- [ ] 3.11 Implement `SIGINT` and `SIGTERM` handling that terminates tracked active provider children before exiting on supported Unix targets.

## 4. Rust Provider Adapters

- [ ] 4.1 Implement fixed typed provider adapters for Claude, Cursor, Kimi, and Codex.
- [ ] 4.2 Preserve provider capability metadata returned by `providers_list`.
- [ ] 4.3 Preserve provider-specific mode and option validation for `effort` and `thinking`.
- [ ] 4.4 Preserve provider command construction, prompt rendering, cwd, timeout, model, and provider-specific args.
- [ ] 4.5 Preserve provider environment allowlists, including Claude `ANTHROPIC_BASE_URL` stripping.
- [ ] 4.6 Preserve provider version command and startup smoke command behavior.
- [ ] 4.7 Implement the public `providers_check` tool dispatch using adapter-owned version and smoke commands.

## 5. Rust Task Lifecycle

- [ ] 5.1 Implement registry load/save with atomic writes on supported targets and compatibility for current Node-created task records.
- [ ] 5.2 Keep Rust registry writes Node-readable until the final entrypoint switch; gate any versioned migration behind an explicit post-switch migration flag.
- [ ] 5.3 Add a rollback fixture that writes state with the Rust binary, starts the Node server against that state directory, and verifies tasks remain inspectable.
- [ ] 5.4 Preserve the on-disk task layout `stateDir/tasks/<taskId>/stdout.log`, `stderr.log`, and `result.json` so rollback and inspection remain compatible.
- [ ] 5.5 Serialize registry writes through the actor or a single writer queue so concurrent task completions cannot overlap atomic writes.
- [ ] 5.6 Make persisted registry deserialization tolerant of unknown fields while keeping public tool inputs strict.
- [ ] 5.7 Keep task IDs in the existing `task_` plus UUID-hex shape and retry if a generated ID collides with persisted state.
- [ ] 5.8 Clean up or ignore known temporary registry files from crashed atomic writes before loading registry state.
- [ ] 5.9 Define behavior for corrupted canonical `registry.json` and add a fixture test for that edge case.
- [ ] 5.10 Implement the task-manager actor so task registry and active task access are serialized.
- [ ] 5.11 Ensure the actor never awaits provider processes, git commands, log drains, or worktree cleanup directly; background tasks must send completion messages back to the actor.
- [ ] 5.12 Ensure actor panic fails the server fast instead of leaving request handlers waiting indefinitely.
- [ ] 5.13 Add explicit state transition functions for queued, running, succeeded, failed, stopped, failed_stale, and removed.
- [ ] 5.14 Implement startup stale recovery for previously queued or running tasks.
- [ ] 5.15 Implement spawn, preview, list, status, wait, logs, result, stop, and remove behavior.
- [ ] 5.16 Preserve current `task_remove` behavior by rejecting queued or running tasks until callers stop them or they reach a final state.
- [ ] 5.17 Implement stdout/stderr capped logs while continuing to drain provider pipes after the cap.
- [ ] 5.18 Decode provider stdout/stderr lossy for invalid UTF-8 to preserve Node-compatible log behavior.
- [ ] 5.19 Implement timeout, stop, provider start error, provider exit error, and stale error type recording.
- [ ] 5.20 Implement git status, git diff, changed files, worktree creation, and managed worktree cleanup.

## 6. Packaging And Entrypoint

- [ ] 6.1 Define supported release targets and document provider CLI limitations per platform.
- [ ] 6.2 Add a built binary smoke test for `initialize`, `tools/list`, `providers_list`, `providers_check`, and `task_preview`.
- [ ] 6.3 Audit public tool names, input schemas, defaults, response shapes, state paths, and install commands; document either "no break" or exact migration notes.
- [ ] 6.4 Implement the first install path as direct prebuilt binary releases for supported targets.
- [ ] 6.5 During transition, expose the Rust binary under a distinct command such as `agent-bridge-mcp-rs`.
- [ ] 6.6 After parity is proven, switch the final `agent-bridge-mcp` MCP entrypoint to the Rust binary.
- [ ] 6.7 If npm install UX is retained, implement npm only as a platform-specific prebuilt binary launcher and verify it with installed-entrypoint smoke tests.
- [ ] 6.8 Update README and example MCP config for the final binary entrypoint.

## 7. Verification

- [ ] 7.1 Run existing Node tests while the Node implementation remains present.
- [ ] 7.2 Run `cargo test` for Rust unit and integration tests.
- [ ] 7.3 Run stdio golden fixtures against both Node and Rust and confirm public response parity.
- [ ] 7.4 Run fake-provider lifecycle tests against the Rust binary.
- [ ] 7.5 Run built or installed binary smoke tests.
- [ ] 7.6 Run `cargo clippy` or an equivalent Rust lint gate that rejects stringly `serde_json::Value` use on tool dispatch hot paths unless explicitly justified.
- [ ] 7.7 Run `openspec validate port-mcp-to-rust-binary` and confirm the change remains valid.
