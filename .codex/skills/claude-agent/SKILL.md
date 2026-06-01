---
name: claude-agent
description: Direct Claude CLI runbook for one-shot provider use outside Agent Bridge.
provider_id: claude
provider_cli: claude
supported_modes:
  - research
  - review
  - implement
  - command
---

# Claude Agent

Use this skill only when an operator explicitly wants to run Claude directly. Prefer Agent Bridge for delegated background tasks, managed worktree isolation, readiness checks, log polling, diffs, and final task metadata.

## Install Check

Run `claude --version` for the native CLI, or `claude-p --version` when using the local wrapper. If Agent Bridge is the caller, prefer `doctor`, `providers_check`, and `task_preview`.

## Safe Default Invocation

For read-only direct review, run:

```bash
claude -p --output-format json --permission-mode dontAsk --allowedTools Read,Grep,Glob --disallowedTools Bash,Edit,Write "$PROMPT"
```

Inspect stdout, stderr, JSON output, and exit status.

## Dangerous Flags

Write-capable tool access requires explicit user authorization before use. `--permission-mode default`, `--permission-mode autoEdit`, `--permission-mode autoEditWithFullAuto`, broad `--allowedTools`, omitted `--disallowedTools`, and any unattended write behavior require explicit user authorization before use.

## Safety Constraints

Keep direct prompts scoped to the current repository. For implementation work, prefer Agent Bridge `task_spawn` with `isolation: "worktree"` so edits, logs, and diffs are inspectable before integration.

## Agent Bridge Mode Mapping

- `research`: direct Claude should use `Read,Grep,Glob` only.
- `review`: direct Claude should use `Read,Grep,Glob` only.
- `implement`: prefer Agent Bridge managed worktree isolation before direct edits.
- `command`: prefer Agent Bridge unless the user requested a one-shot shell-capable call.

## Evidence Expectations

After the subprocess exits, inspect exit status, stdout, stderr, changed files, and JSON output. The main caller still runs the smallest relevant proof before claiming work complete.

## Troubleshooting

If auth or Keychain access fails under Agent Bridge, use the Claude host-runner workflow. If direct Claude works but Agent Bridge does not, compare `doctor`, `providers_check`, and `task_preview` output before changing permissions.

## Agent Bridge Boundary

This skill documents direct Claude CLI use. Agent Bridge provider adapters remain the runtime authority for command construction, environment policy, workspace isolation, readiness checks, task lifecycle state, and result inspection.
