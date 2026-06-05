## 1. Research and Characterization

- [x] 1.1 Read and summarize the current Claude Code CLI, hooks, settings, and upstream `claude-p` docs into implementation notes inside the change.
- [x] 1.2 Inspect upstream `claude-p` source for PTY startup, terminal probe responses, hook command protocol, transcript parsing, timeout handling, and cleanup behavior; record findings in `upstream-claude-p-notes.md`.
- [x] 1.3 Verify interactive Claude flags for mode mapping, including `--permission-mode`, `--allowedTools`, `--disallowedTools`, `--settings`, `--model`, and `--effort`; record the matrix before implementation.
- [x] 1.4 Enumerate current Claude StopFailure input error strings and setup prompt signatures from docs, source behavior, and live/fake fixtures before mapping implementation.
- [x] 1.5 Evaluate Rust PTY crate options against macOS/Linux support, Tokio integration, process cleanup, and maintenance risk; document the selected dependency or adapter approach before adding it.
- [x] 1.6 Add deterministic fake interactive-Claude fixtures for terminal probes, prompt entry, Stop-hook payloads, StopFailure payloads, transcript files, malformed transcript, first-run prompts, timeout, and child cleanup. This is blocked on the PTY adapter choice in 1.5.
- [x] 1.7 Add the PTY adapter dependency and a focused spike test using fake interactive Claude to verify split read/write, terminal probe bytes, resize, child session/process-group cleanup, and macOS/Linux behavior before production runner wiring.
- [x] 1.8 Record the protocol v2 schema, runner-result integration contract, hook relay contract, startup sequencing, and login-shell bootstrap decision from Gate 3 review.

## 2. Owned Runner Implementation

- [x] 2.1 After task 1.7 passes, add an owned Claude runner module that starts official interactive `claude` in a PTY with the documented login-shell-compatible environment setup.
- [x] 2.2 Implement terminal probe detection/responses needed for Claude Code startup without writing probe noise to MCP stdout.
- [x] 2.3 Implement prompt injection through PTY input and ensure rendered task prompts never appear in process argv or diagnostics.
- [x] 2.4 Generate temporary runner-owned `--settings` JSON for `SessionStart`, `Stop`, and `StopFailure` hooks without editing durable Claude config.
- [x] 2.5 Implement runner-owned hook relay IPC per `hook-relay-contract.md`, including owner-only permissions, bounded reads, cleanup, and no hook stdout leakage into Claude context.
- [x] 2.6 Parse Stop-hook payloads, validate transcript paths, read transcript JSONL with bounded retry, extract final assistant output, and map it into existing provider result/log surfaces.
- [ ] 2.7 Parse StopFailure hook payloads and map known Claude API/auth/billing/rate-limit failures to actionable provider diagnostics.
- [ ] 2.8 Detect first-run/login/setup prompts and fail fast with actionable diagnostics.
- [ ] 2.9 Implement timeout, client-disconnect, shutdown, and PTY process-tree cleanup for child processes.

## 3. Provider and Host-Runner Integration

- [ ] 3.1 Implement host-runner protocol v2 per `protocol-v2.md`, including rejected legacy shapes, response schema validation, and protocol mismatch handling.
- [ ] 3.2 Replace Claude provider command selection so normal tasks and smoke checks use the owned interactive runner launch strategy.
- [ ] 3.3 Remove native `claude -p` and upstream `claude-p` fallback selection from Claude provider execution and diagnostics.
- [ ] 3.4 Update Claude binary resolution and environment allowlists so the official interactive `claude` binary is explicit and `CLAUDE_P_BIN` is ignored legacy configuration.
- [ ] 3.5 Replace print-mode stdout JSON success parsing and smoke-token detection with structured v2 owned-runner Stop/transcript result parsing per `runner-result-contract.md`.
- [ ] 3.6 Update provider metadata, previews, readiness diagnostics, doctor output, bare profile diagnostics, and launch strategy reporting to describe the owned interactive runner.
- [ ] 3.7 Ensure version-only checks never mark Claude launchable; only owned-runner host-runner smoke success can do that.

## 4. Docs, Validation, and Release Plumbing

- [ ] 4.1 Update README setup and troubleshooting to remove print-mode fallback guidance and document the owned runner workflow, env migration, host-runner requirement, smoke duration expectations, and rollback shape.
- [ ] 4.2 Update guidance specs/docs that mention `claude-p`, native `claude -p`, or direct Claude launch strategy.
- [ ] 4.3 Add or update tests for provider readiness, task preview redaction, host-runner safety, stdout isolation, transcript parsing, StopFailure handling, bare profile hooks, and no-fallback behavior.
- [ ] 4.4 Run OpenSpec validation and the relevant Rust test suite.
- [x] 4.5 Re-submit the revised plan for Agent Bridge review before implementation tasks beyond research.
- [ ] 4.6 Build the release binary, replace the installed Agent Bridge binary, and verify the installed MCP tool surface.
- [ ] 4.7 Run an optional live Claude smoke check through the owned host runner when local Claude auth is available.
