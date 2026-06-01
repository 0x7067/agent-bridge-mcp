## Context

The bridge currently selects `claude-p` by default for the Claude provider, unless native `CLAUDE_BIN` is explicitly configured and `CLAUDE_P_BIN` is not set. The selected Claude command is wrapped through `/bin/zsh -lc` so shell initialization can make local CLI auth and PATH behavior match an interactive terminal.

This default is fragile because upstream `claude-p` is a compatibility wrapper around interactive Claude Code. Its own documentation describes PTY/terminal handling, Claude Code Stop hook integration, macOS/Linux focus, and sensitivity to Claude Code behavior changes. The bridge must treat `claude-p` as an external integration boundary, not as a simple JSON CLI.

The main failure modes to design for are:

- `claude-p` exists and answers a version probe but hangs or fails when executing a task.
- Claude Code emits terminal/probe/control output that is not the final task result.
- Stop hook payloads are missing or changed, leaving the wrapper without a parseable result.
- Shell initialization or host environment differences make MCP behavior diverge from a user's terminal.
- Positional prompt transport breaks on large prompts, multiline prompts, leading dashes, or shell-sensitive content.
- Native `claude -p` is available and more stable for a user's setup, but the bridge gives no clear fallback guidance.

## Goals / Non-Goals

**Goals:**
- Make Claude provider health checks reflect real task readiness, not just binary presence.
- Add deterministic fake-provider coverage for unreliable `claude-p` behaviors without requiring live Claude auth in CI.
- Surface concise diagnostics that help users decide whether to fix `claude-p`, switch to native `claude -p`, or adjust environment configuration.
- Keep provider stdout/stderr isolated from MCP protocol stdout.
- Preserve the public MCP tool surface and current task lifecycle shape unless a small additive diagnostic field is needed.
- Feed the final behavior into the Rust port as a compatibility contract.

**Non-Goals:**
- Do not vendor, patch, or fork upstream `claude-p`.
- Do not make live Claude Code execution a required automated test.
- Do not silently switch a user's Claude provider command after a failed smoke probe.
- Do not expose prompts, tokens, or full process environments in diagnostics.
- Do not solve general provider reliability for Cursor, Kimi, or Codex in this change.

## Decisions

### Decision 1: Treat `claude-p` health as task-path readiness

`providers_check` already has a smoke mode. This change will make the Claude smoke path the primary readiness signal because the version path proves only that an executable exists. The smoke probe must use the same adapter command builder, shell initialization, environment policy, and output parsing as a real Claude task.

Alternative considered: keep version checks as the primary health indicator and add docs only. That would preserve the current false-positive failure mode where `claude-p --version` passes but real task execution still hangs or returns no result.

### Decision 2: Use fake `claude-p` fixtures for the reliability test matrix

Tests will add fixture binaries/scripts that emulate failure modes: hanging, terminal noise, malformed output, missing result, non-zero exit, delayed output, prompt echo, and native Claude override. These tests are deterministic and do not require Claude credentials, network access, or a local Claude Code installation.

Alternative considered: run live `claude-p` in automated tests. That would catch real upstream changes but is not suitable as a default test because it depends on user auth, local setup, and possible paid model usage.

### Decision 3: Keep fallback advisory before automatic fallback

When `claude-p` fails but native `claude -p` is discoverable or configured, `providers_check(smoke: true)` should recommend the native path. It should not silently mutate future task execution because silent fallback can change permissions, output behavior, and user expectations.

Alternative considered: automatically prefer native `claude -p` whenever it exists. That may be better long-term, but it is a provider policy change with compatibility risk. The initial hardening should first make health and diagnostics trustworthy.

### Decision 4: Make diagnostics classified, bounded, and redacted

Claude failures should be categorized with stable labels such as `provider_timeout`, `provider_start_error`, `provider_exit_error`, and `provider_output_error`. Diagnostics should include provider label, selected command label, timeout, exit code when known, and capped stdout/stderr excerpts. They must not include raw prompts, API tokens, OAuth tokens, or unallowlisted environment values.

Alternative considered: return raw logs only. Raw logs are useful during local debugging but unsafe as a default response shape and too inconsistent for reliable tests.

The implementation should converge on an additive diagnostic shape close to:

```json
{
  "failureCategory": "provider_timeout",
  "provider": "claude",
  "commandKind": "claude-p",
  "commandPath": "claude-p",
  "startupVerified": false,
  "timeoutMs": 5000,
  "exitCode": null,
  "signal": "SIGTERM",
  "stdoutExcerpt": "capped provider stdout",
  "stderrExcerpt": "capped provider stderr",
  "recommendation": "Set CLAUDE_BIN to native claude -p and retry providers_check with smoke: true"
}
```

`stdoutExcerpt` and `stderrExcerpt` should be capped to a small fixed size, with tests pinning the cap. The shape must be available from both `providers_check(smoke: true)` and failed Claude `task_result` responses; task logs may include the same payload as an additional troubleshooting aid. The shape can be nested under existing response fields as long as it remains additive and redacted.

### Decision 5: Require non-argv prompt transport

The current adapter passes the rendered prompt as a positional argument after `--`. That is not acceptable for this change because local process listings can expose argv values, and task prompts are explicitly treated as sensitive diagnostic data. The implementation phase must verify from upstream `claude-p` and Claude Code behavior whether stdin or input-file transport is supported. The preferred order is stdin first, input-file second, and no positional argv prompt transport. If input-file transport is used, temporary files must be created with owner-only permissions, cleaned up reliably, and excluded from diagnostics. If the selected Claude path cannot transport prompts without argv exposure, the bridge should reject before spawning with an actionable error rather than launch a process that leaks prompt text.

Alternative considered: keep positional arguments for small prompts because they are simple. That leaves sensitive prompt text visible to local process inspection and conflicts with the redaction requirement.

### Decision 6: Keep public APIs stable with additive diagnostics only

The public MCP tools remain unchanged. If richer diagnostics need to be returned, they should be additive fields in existing responses or logs. Existing clients that ignore unknown fields should continue to work.

Alternative considered: add a new `claude_diagnose` tool. That may be useful later, but the immediate reliability issue already flows through `providers_check`, `task_logs`, and `task_result`.

## Risks / Trade-offs

- `claude-p` upstream behavior changes again -> Mitigation: keep fake fixtures focused on bridge contracts and document upstream compatibility links; make live smoke optional/manual.
- Diagnostics leak sensitive data -> Mitigation: redact prompts and credentials by construction, add tests for redaction, cap stdout/stderr excerpts.
- Prompt text leaks through local process listings -> Mitigation: require stdin/input-file or another non-argv transport for all Claude task prompts; reject before spawn when unsupported.
- Smoke probes spend user quota or take too long -> Mitigation: keep smoke opt-in through `providers_check(smoke: true)`, honor caller timeout, and use a minimal prompt.
- Shell initialization remains a source of nondeterminism -> Mitigation: test shell-wrapped command construction and report command selection clearly; avoid exposing full environment.
- Native Claude fallback differs from `claude-p` behavior -> Mitigation: recommend rather than silently switch in this change, and document the trade-off.
- Claude reliability behavior drifts during future refactors -> Mitigation: make this spec testable and reference it from Rust implementation tasks once accepted.

## Migration Plan

1. Characterize the current Rust Claude provider path with fake fixtures before changing behavior.
2. Add characterization tests against the unmodified adapter and record the failing baseline for each current reliability/security gap.
3. Add failure classification and bounded diagnostics for provider version checks, smoke checks, task exits, timeouts, and malformed output.
4. Add prompt transport tests and update command construction only after verifying upstream-supported non-argv transport.
5. Add advisory fallback reporting for native `claude -p`.
6. Update README troubleshooting and provider setup docs.
7. Run unit tests, OpenSpec validation, and optional manual live smoke checks where local Claude auth is available.

Rollback is straightforward because this change is confined to provider adapter behavior, diagnostics, tests, and docs. If a diagnostic shape causes compatibility issues, keep the underlying hardening and remove or rename only the additive diagnostic fields before release.

## Open Questions

- Does upstream `claude-p` currently support stdin or input-file prompt transport in the installed versions this project needs to support, and if not, should this project require native `claude -p` for Claude provider task execution?
- Should native `claude -p` become the default in a later change if smoke data shows it is consistently more reliable than `claude-p`?
- None.
