---
name: pi-agent
description: Direct Pi CLI runbook for the Kimi-backed provider outside Agent Bridge.
provider_id: kimi
provider_cli: pi
pinned_model: accounts/fireworks/routers/kimi-k2p6-turbo
supported_modes:
  - research
  - review
  - implement
  - command
---

# Pi Agent

Use this skill only when an operator explicitly wants to run the `pi` CLI directly. Agent Bridge provider id `kimi` maps to the local `pi` command for runtime task execution.

## Install Check

Run `pi --version` before direct use. If Agent Bridge is the caller, prefer `doctor`, `providers_check`, and `task_preview` to inspect command, tools, thinking level, and prompt.

## Safe Default Invocation

For read-only direct review with the pinned Kimi model, run:

```bash
pi -p --model accounts/fireworks/routers/kimi-k2p6-turbo --no-session --no-context-files --tools read,grep,find,ls "$PROMPT"
```

Inspect stdout, stderr, and exit status before trusting the answer.

## Dangerous Flags

Write-capable tool sets require explicit user authorization before use. `--tools` values that include `bash`, `edit`, `write`, broad filesystem access, and unattended write behavior require explicit user authorization before use. Do not enable write tools for direct runs when Agent Bridge managed worktree isolation would provide safer review.

## Safety Constraints

Keep direct `pi` runs scoped to the requested repository. For implementation work, prefer Agent Bridge `task_spawn` with `provider: "kimi"` and `isolation: "worktree"` so edits and diffs can be inspected before integration.

## Agent Bridge Mode Mapping

- `research`: direct `pi` should use read-only tools such as `read,grep,find,ls`.
- `review`: direct `pi` should use read-only tools such as `read,grep,find,ls`.
- `command`: use bounded shell-capable tools only with explicit user authorization.
- `implement`: prefer Agent Bridge managed worktree isolation before enabling edit or write tools directly.

## Evidence Expectations

After the subprocess exits, inspect stdout, stderr, exit code, changed files, and diffs. The main caller still runs the smallest relevant proof before claiming work complete.

## Troubleshooting

If `pi` cannot find the pinned model, inspect `pi` model configuration and credentials, then refresh `pinned_model` in this repo-owned skill after confirming the replacement model locally. If Agent Bridge Kimi fails, compare `providers_check`, `task_preview`, tool selection, thinking level, and environment before changing direct CLI flags.

## Agent Bridge Boundary

This skill documents direct `pi` CLI use for the Kimi-backed provider. Agent Bridge provider adapters remain the runtime authority for command construction, environment policy, readiness checks, task lifecycle state, and result inspection.
