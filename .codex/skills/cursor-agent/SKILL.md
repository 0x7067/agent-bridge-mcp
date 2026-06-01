---
name: cursor-agent
description: Direct Cursor Agent CLI runbook for one-shot provider use outside Agent Bridge.
provider_id: cursor
provider_cli: cursor-agent
supported_modes:
  - research
  - review
  - implement
---

# Cursor Agent

Use this skill only when an operator explicitly wants to run Cursor Agent directly. Prefer Agent Bridge for delegated background tasks, managed worktree isolation, readiness checks, log polling, diffs, and final task metadata.

## Install Check

Run `cursor-agent --version` before direct use. If Agent Bridge is the caller, prefer `doctor`, `providers_check`, and `task_preview` to inspect command, workspace, trust mode, and prompt.

## Safe Default Invocation

For read-only direct review, run:

```bash
cursor-agent -p --output-format json --mode ask --workspace "$PWD" -- "$PROMPT"
```

Inspect JSON output, stderr, and exit status before trusting the answer.

## Dangerous Flags

Write-capable mode requires explicit user authorization before use. `--trust`, `--force`, `--yolo`, broad filesystem access, and unattended write behavior require explicit user authorization before use. Do not treat trust mode as permission to expand scope beyond the requested repository.

## Safety Constraints

Cursor Agent does not support Agent Bridge `command` mode. For implementation work, prefer Agent Bridge `task_spawn` with `isolation: "worktree"` so edits and diffs are isolated before review.

## Agent Bridge Mode Mapping

- `research`: direct Cursor Agent should use `--mode ask`.
- `review`: direct Cursor Agent should use `--mode ask`.
- `implement`: prefer Agent Bridge managed worktree isolation before direct write-capable use.

## Evidence Expectations

After the subprocess exits, inspect JSON output, stderr, exit code, changed files, and diffs. The main caller still runs the smallest relevant proof before claiming work complete.

## Troubleshooting

If Cursor Agent fails under Agent Bridge, compare `providers_check`, `task_preview`, workspace path, and trust configuration before changing direct CLI flags. If direct Cursor succeeds but Agent Bridge fails, keep runtime debugging in Agent Bridge diagnostics first.

## Agent Bridge Boundary

This skill documents direct Cursor Agent CLI use. Agent Bridge provider adapters remain the runtime authority for command construction, workspace isolation, readiness checks, task lifecycle state, and result inspection.
