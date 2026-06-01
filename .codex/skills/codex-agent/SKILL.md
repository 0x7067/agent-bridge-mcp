---
name: codex-agent
description: Direct Codex CLI runbook for one-shot provider use outside Agent Bridge.
provider_id: codex
provider_cli: codex
supported_modes:
  - research
  - review
  - implement
  - command
---

# Codex Agent

Use this skill only when an operator explicitly wants to run Codex directly. Prefer Agent Bridge for delegated background tasks, managed worktree isolation, readiness checks, log polling, diffs, and final task metadata.

## Install Check

Run `codex --version` before direct use. If Agent Bridge is the caller, prefer `doctor`, `providers_check`, and `task_preview` to inspect command, cwd, sandbox, and prompt transport.

## Safe Default Invocation

For read-only direct review, run:

```bash
codex exec --json --sandbox read-only "$PROMPT"
```

Inspect JSON events, stderr, and exit status.

## Dangerous Flags

Write-capable sandbox or approval changes require explicit user authorization before use. `--sandbox workspace-write`, broader filesystem access, auto-approval, and unattended write behavior require explicit user authorization before use. Do not loosen sandbox settings after a denial until cwd, workspace policy, prompt scope, and isolation are understood.

## Safety Constraints

Keep write-capable direct Codex runs inside the intended repository. For implementation work, prefer Agent Bridge `task_spawn` with `isolation: "worktree"` so sandbox denials, logs, diffs, and final diagnostics are captured.

## Agent Bridge Mode Mapping

- `research`: direct Codex should use `--sandbox read-only`.
- `review`: direct Codex should use `--sandbox read-only`.
- `implement`: prefer Agent Bridge managed worktree isolation before workspace writes.
- `command`: keep prompts bounded and inspect JSON events plus stderr.

## Evidence Expectations

After the subprocess exits, inspect JSON events, stderr, exit code, and repository diff. The main caller still runs the smallest relevant proof before claiming work complete.

## Troubleshooting

For `patch rejected`, sandbox denial, approval denial, outside-project, or out-of-workspace write symptoms, inspect cwd, workspace policy, prompt scope, and isolation strategy. In Agent Bridge, use `task_wait`, `task_logs`, `task_status`, and final `task_result`.

## Agent Bridge Boundary

This skill documents direct Codex CLI use. Agent Bridge provider adapters remain the runtime authority for command construction, sandbox selection, environment policy, readiness checks, task lifecycle state, and result inspection.
