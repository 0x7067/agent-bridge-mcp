## 1. Baseline Investigation

- [ ] 1.1 Read the current Claude adapter command, environment, provider check, process runner, and task result code paths.
- [ ] 1.2 Verify upstream `claude-p` invocation, prompt transport, Stop hook, PTY, and compatibility behavior from the upstream README/SPEC and the locally installed version if present.
- [ ] 1.3 Document current behavior for `CLAUDE_P_BIN`, `CLAUDE_BIN`, shell initialization, `providers_check`, `providers_check(smoke: true)`, and task result parsing.
- [ ] 1.4 Decide whether prompt transport can be safely changed to stdin/input-file for `claude-p`, native `claude -p`, both, or neither.

## 2. Fake Provider Fixtures

- [ ] 2.1 Add test fixture scripts for fake `claude-p` success, version-only success, startup hang, delayed timeout, non-zero exit, malformed output, terminal noise, missing result, and prompt echo.
- [ ] 2.2 Add a fake native `claude` fixture that exercises `CLAUDE_BIN` selection and native `-p` argument behavior.
- [ ] 2.3 Add helpers that run fixture providers through the real adapter command path, including `/bin/zsh -lc` wrapping where applicable.

## 3. Characterization Tests

- [ ] 3.1 Add tests proving Claude version checks report binary presence without claiming startup readiness, and run them against the unmodified adapter to capture the failing or passing baseline.
- [ ] 3.2 Add tests proving Claude smoke checks exercise the same selected command path as real smoke tasks, and run them against the unmodified adapter to capture the failing or passing baseline.
- [ ] 3.3 Add tests for `claude-p` timeout, non-zero exit, missing result, malformed output, and terminal noise classification, and record the current failing outputs before implementation.
- [ ] 3.4 Add tests proving prompts with newlines, quotes, shell metacharacters, and leading dashes arrive as prompt data rather than flags or shell-expanded text.
- [ ] 3.5 Add tests proving Claude prompts are not present in the spawned provider argv for either `claude-p` or native `claude -p`.
- [ ] 3.6 Add tests proving diagnostics redact prompts, API tokens, OAuth tokens, and non-allowlisted environment values.
- [ ] 3.7 Add tests proving provider stdout/stderr never reaches MCP server stdout.

## 4. Diagnostics And Health Implementation

- [ ] 4.1 Introduce stable Claude failure categories for timeout, start failure, exit failure, and output parsing failure.
- [ ] 4.2 Define the additive diagnostic payload fields, nesting location, stdout/stderr byte cap, and redaction rules before changing task/result behavior.
- [ ] 4.3 Add bounded, redacted diagnostic fields to both `providers_check(smoke: true)` and failed Claude `task_result` responses; task logs may append the same payload.
- [ ] 4.4 Ensure `providers_check` without smoke reports `startupVerified: false` for Claude and does not imply task readiness.
- [ ] 4.5 Ensure Claude smoke checks use adapter-owned command selection, shell initialization, environment policy, timeout behavior, and output parsing.
- [ ] 4.6 Add native `claude -p` recommendation diagnostics when `claude-p` smoke fails and native Claude is configured or discoverable.

## 5. Prompt Transport Hardening

- [ ] 5.1 Update Claude command construction to use stdin, input-file, or another verified non-argv prompt transport for all `claude-p` and native `claude -p` task prompts.
- [ ] 5.2 Define the runtime capability detection mechanism for non-argv prompt transport, using an explicit provider capability table or a pre-flight probe.
- [ ] 5.3 Prefer stdin transport, use input-file transport only as a fallback, and explicitly disallow positional argv prompt transport.
- [ ] 5.4 If input-file transport is used, create temp files with `0600` permissions, clean them up reliably, and exclude temp paths from diagnostics.
- [ ] 5.5 Reject tasks before spawn with an actionable validation error when the selected Claude path cannot transport prompts without placing prompt text in argv.
- [ ] 5.6 Preserve current preview redaction behavior after any prompt transport change.

## 6. Documentation

- [ ] 6.1 Update README setup docs for `CLAUDE_P_BIN`, `CLAUDE_BIN`, default command selection, and native fallback guidance.
- [ ] 6.2 Add a Claude troubleshooting section covering `providers_check` with and without `smoke: true`, timeout symptoms, output parse failures, shell initialization, and safe diagnostic collection.
- [ ] 6.3 Link to upstream `claude-p` documentation and explain that Claude Code terminal or Stop hook changes can break the wrapper.
- [ ] 6.4 Confirm the provider adapter contract remains public-API compatible after the Claude prompt transport hardening.

## 7. Verification

- [ ] 7.1 Run `rtk cargo test`.
- [ ] 7.2 Run `rtk openspec validate harden-claude-p-integration`.
- [ ] 7.3 Run a local MCP smoke check for `providers_check` and `providers_check(smoke: true)` using fake Claude fixtures.
- [ ] 7.4 If local Claude auth is available and the user opts in, run an optional live `claude-p` and native `claude -p` smoke comparison and record the result in implementation notes.
- [ ] 7.5 Add or update a short implementation note that pins the accepted Claude provider reliability contract for the Rust port to consume.
- [ ] 7.6 Confirm the Rust implementation references or preserves the accepted Claude provider reliability contract.
