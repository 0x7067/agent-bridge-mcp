## Context

The Rust MCP server already returns enough raw data for inspection: status, `errorType`, logs, diagnostics, git status, diff, changed files, exit code, signal, and truncation flags. The missing layer is a stable, caller-friendly summary that tells the supervising agent what to look at next without pretending that provider completion is project verification.

The host-runner protocol already supports explicit setup, ping, workspace-policy mismatch detection, stale socket handling, and safe shutdown behavior. This change should make that lifecycle easier to discover through MCP guidance and docs rather than introducing a daemon manager.

## Decisions

### Decision 1: Add `reviewPacket` to `task_result`

`task_result` will include a derived `reviewPacket` object for every inspectable task result response. The object will summarize:

- `status`, `phase`, `isFinal`, `provider`, `mode`, `title`, and `cwd`.
- `changedFiles`, `hasChanges`, and a `gitStatusSummary` string derived from existing `gitStatus`.
- `exitCode`, `signal`, `errorType`, and `diagnostic` pass-through fields when present.
- `stdoutTruncated` and `stderrTruncated`.
- `recommendedActions`, an ordered list of short strings such as inspect logs, inspect diff, run verification, stop stalled work, or call `task_remove` after reviewing a managed worktree.

The packet is deliberately derived from existing task state and result payloads. It does not parse provider prose, judge correctness, or claim tests have passed.

### Decision 2: Extend guidance instead of adding host-runner management

Host-runner lifecycle improvements will be implemented as discoverable prompts/resources and README guidance. They will cover start, ping/readiness, restart after workspace changes, stop, stale socket behavior, and known failure diagnostics. A first-class service manager is out of scope for this change.

### Decision 3: Dogfood workflows are reproducible guidance, not CI live smokes

Dogfood workflows will be checked into MCP guidance/resources and README examples. They will use small prompts, bounded waits, `isolation: "none"` for read-only work, `isolation: "worktree"` for implementation work, and explicit `task_result` review. They will not run live Claude/Cursor/Kimi/Codex tasks in the default test suite.

## Risks

- `reviewPacket` could become a second source of truth. Keep it additive and derived from the existing public result fields.
- Guidance could imply provider output is sufficient proof. Keep every workflow explicit that the main caller runs final verification.
- Host-runner docs could drift from behavior. Tests should assert the guidance resources/prompts are listed and include the important lifecycle terms.

## Rollout

This is an additive change. Existing callers can ignore `reviewPacket`; callers that want a concise inspection summary can start reading it from `task_result`.
