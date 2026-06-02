## Reduced-Profile Spike Findings

Checked on 2026-06-01 from `/Users/pedro/Development/agent-bridge-mcp`.

### Provider Versions

| Provider | Version evidence |
| --- | --- |
| Claude | `claude-p 0.1.0` |
| Codex | `codex-cli 0.135.0` |
| Cursor | `2026.05.28-a70ca7c` |
| Kimi/Pi | `0.78.0` |

### Capability Matrix

| Capability | Claude (`claude-p`) | Codex | Cursor | Kimi/Pi |
| --- | --- | --- | --- | --- |
| Compact task prompt | supported | supported | supported | supported |
| Custom system prompt | supported via `--system-prompt` | best_effort via compact user prompt/config only | unsupported in public help | supported via `--system-prompt` |
| Disable hooks | best_effort via `--setting-sources` and wrapper-controlled settings; wrapper still injects Stop hook | unsupported; `--dangerously-bypass-hook-trust` trusts hooks, it does not disable them | unsupported in public help | supported for extension/prompt surfaces; no explicit hook concept in help |
| Disable skills/rules | best_effort via settings source restriction; no explicit no-skills flag in wrapper help | supported for rules via `--ignore-rules`; skills are not a Codex exec CLI concept in help | best_effort by avoiding `--plugin-dir`; Cursor may still load workspace/user rules | supported via `--no-skills` and `--no-prompt-templates` |
| Disable context files | best_effort via compact prompt/settings; no explicit no-context flag in wrapper help | best_effort via `--ignore-rules`; no explicit no-context-files flag in help | best_effort via mode/workspace only; no explicit no-context flag in help | supported via `--no-context-files` |
| Isolate config | best_effort via `--setting-sources` and controlled cwd/settings | supported via `--ignore-user-config`; auth still uses `CODEX_HOME` | best_effort via workspace/plugin controls; no ignore-user-config flag in help | supported via `PI_CODING_AGENT_DIR`, `--session-dir`, and `--no-*` discovery flags |
| Disable memory/session reuse | supported by not passing resume/continue/session args | supported via `--ephemeral` | supported by not passing `--resume`/`--continue` | supported via `--no-session` |
| Preserve auth under reduced config | likely supported; wrapper remains authenticated through normal Claude CLI auth | supported: help says `--ignore-user-config` does not replace auth source | supported if normal Cursor auth is available | supported through API key/env auth |
| Evidence method | `claude-p --help`, version probe, existing host-runner smoke | `codex exec --help`, version probe, existing smoke | `cursor-agent --help`, version probe, existing smoke | `pi --help`, version probe, existing smoke |

### Provider Notes

#### Claude

`claude-p --help` exposes `--system-prompt`, `--append-system-prompt`, `--setting-sources`, `--allowedTools`, `--disallowedTools`, `--permission-mode`, and session resume controls. The wrapper injects a Stop hook to emulate print mode, so `bare` cannot honestly mean "no hooks at all" for `claude-p`. Mark hook disabling as `best_effort` and report the wrapper Stop hook caveat in profile diagnostics.

#### Codex

`codex exec --help` exposes the strongest reduced-profile controls: `--ignore-user-config`, `--ignore-rules`, and `--ephemeral`, plus sandbox and config overrides. Use these in `bare` profile. There is no direct `--system-prompt` flag in `codex exec --help`, so compact user/task prompt is the reliable path.

#### Cursor

`cursor-agent --help` exposes `--print`, `--output-format`, `--mode`, `--workspace`, `--trust`, `--sandbox`, `--plugin-dir`, worktree controls, and resume/continue controls. It does not expose system-prompt, no-skills, no-rules, no-context, or ignore-user-config flags. `bare` should be best-effort: compact prompt, avoid plugin dirs, avoid resume/continue, select read-only modes where appropriate, and report unsupported reductions.

#### Kimi/Pi

`pi --help` exposes explicit controls for `--system-prompt`, `--no-session`, `--no-tools`, `--tools`, `--no-extensions`, `--no-skills`, `--no-prompt-templates`, `--no-themes`, `--no-context-files`, `--session-dir`, `PI_CODING_AGENT_DIR`, and `PI_CODING_AGENT_SESSION_DIR`. It also emitted a sandbox warning while trying to create a settings lock, so implementation should avoid relying on global settings and should prefer the explicit no-discovery flags plus isolated dirs for `bare`.

### Decision

Keep the profile name `bare`, but all public metadata must make clear it means provider-specific reduced configuration. The actual guarantee is the per-provider `profileDiagnostics` payload, not the profile name.

### Re-Run Checklist

Re-run this spike when upgrading any supported provider CLI or wrapper:

1. Capture `--version`.
2. Capture relevant `--help` output.
3. Re-run `providers_check` with smoke enabled.
4. Re-run focused `task_preview` assertions for `bridge` and `bare`.
5. Update this matrix and provider capability metadata if flags or semantics changed.
