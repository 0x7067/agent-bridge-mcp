## Claude Interactive Flag Matrix

Verified against:

- Installed CLI: `/Users/pedro/.local/bin/claude`
- Installed version: `2.1.165 (Claude Code)`
- Command: `claude --help`
- Official docs: Claude Code CLI usage, hooks reference, and permissions
  reference at `https://code.claude.com/docs/en/cli-usage`,
  `https://code.claude.com/docs/en/hooks`, and
  `https://code.claude.com/docs/en/permissions`

No live prompt or model call was run for this matrix.

## Flag Support

| Flag | Installed CLI support | Interactive? | Notes for owned runner |
| --- | --- | --- | --- |
| `--permission-mode <mode>` | Yes: `acceptEdits`, `auto`, `bypassPermissions`, `default`, `dontAsk`, `plan` | Yes | Use explicit mode per Agent Bridge task mode. Avoid `bypassPermissions` outside controlled sandboxes. |
| `--allowedTools` / `--allowed-tools` | Yes | Yes | Auto-approves matching tool calls; it does not make the listed tools the only available tools. Pair with deny rules or `--tools` for restriction. |
| `--disallowedTools` / `--disallowed-tools` | Yes | Yes | Denies tool rules; bare tool names remove the tool from model context. Deny rules win over allow rules by settings precedence. |
| `--tools <tools...>` | Yes | Yes | Not in the original task list, but relevant: use when the runner needs an allow-only available tool set. |
| `--settings <file-or-json>` | Yes | Yes | Accepts path or JSON string. Use runner-owned temporary settings to register hooks without durable config edits. |
| `--setting-sources <sources>` | Yes | Yes | Accepts comma-separated `user`, `project`, `local`. Bare profile can restrict this to reduce inherited config. |
| `--model <model>` | Yes | Yes | Pass through when Agent Bridge request includes a model. Do not invent fallback model behavior for interactive runs. |
| `--effort <level>` | Yes: `low`, `medium`, `high`, `xhigh`, `max` | Yes | Pass through only after validating against this set. Available levels still depend on model. |
| `--output-format` | Yes | No; help says print-only | Do not use for owned interactive runner. Stop/transcript parsing replaces stdout JSON parsing. |
| `--fallback-model` | Yes | Print/background only | Do not rely on it for interactive provider behavior. |
| `-p` / `--print` | Yes | No | Explicitly out of scope for this provider. |

## Mode Mapping Before Implementation

| Agent Bridge mode | Claude flags | Rationale |
| --- | --- | --- |
| `research` | `--permission-mode dontAsk`, `--tools Read,Grep,Glob`, `--allowedTools Read,Grep,Glob`, `--disallowedTools Bash,Edit,Write` | Read-only analysis should not prompt for known read tools, should not expose or permit shell/edit/write tools, and should fail closed for unapproved tools. |
| `review` | `--permission-mode dontAsk`, `--tools Read,Grep,Glob`, `--allowedTools Read,Grep,Glob`, `--disallowedTools Bash,Edit,Write` | Same as research, with review-specific prompt text handled by Agent Bridge rather than Claude permissions. |
| `command` | `--permission-mode default`, `--tools Read,Grep,Glob,Bash`, `--allowedTools Read,Grep,Glob,Bash`, `--disallowedTools Edit,Write` | Command tasks may need bounded shell evidence but should not edit files through Claude tools. Default mode preserves Claude's normal prompting/risk controls for shell actions not auto-approved by settings. |
| `implement` | `--permission-mode default` | Implementation requires normal read/edit/write behavior under Claude's default permission flow. The runner should not use `bypassPermissions` as a default. |

The current bridge code maps research/review to `dontAsk` with
`allowedTools=Read,Grep,Glob` and `disallowedTools=Bash,Edit,Write`; command to
`default` with read/search/shell allowed and edit/write denied; implement to
`default`. The owned runner should preserve that intent, but add `--tools`
where strict tool exposure is needed because `--allowedTools` is not an
allow-only restriction.

## Settings and Hook Use

Use runner-owned `--settings` to register hook relay commands for
`SessionStart`, `Stop`, and `StopFailure`. The settings payload should be
temporary and per-run; it must not edit `~/.claude/settings.json`, project
settings, or local settings.

For bare profile behavior, `--setting-sources project,local` remains available,
but it is not a full no-hooks guarantee once the runner injects its own Stop
hooks. Diagnostics should describe this honestly as owned-runner hooks plus
reduced inherited settings.

## Implementation Constraints

- Validate `--effort` values before forwarding: `low`, `medium`, `high`,
  `xhigh`, `max`.
- Treat managed settings as potentially stricter than CLI args. A managed deny
  can still block tools even when the runner passes `--allowedTools`.
- Do not place rendered task prompts in argv. Prompt entry belongs on PTY input.
- Do not use `--output-format`, `--input-format`, `--include-partial-messages`,
  or `--permission-prompt-tool` as owned-runner result surfaces; those are
  print-mode semantics.
