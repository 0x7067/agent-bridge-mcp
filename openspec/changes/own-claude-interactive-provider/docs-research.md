## Docs Research Notes

Sources checked while shaping this change:

- Claude Code CLI reference: `claude` starts an interactive session; `claude -p` is the SDK/programmatic query path; interactive flags include `--permission-mode`, `--allowedTools`, `--disallowedTools`, `--settings`, `--model`, `--effort`, and `--name`.
- Claude Code headless/programmatic docs: `claude -p` and Agent SDK usage draw from a separate Agent SDK credit pool starting 2026-06-15, so print-mode fallback is not the provider direction for this bridge.
- Claude Code hooks reference: `SessionStart` fires when a new interactive session starts and includes `transcript_path`; `Stop` includes `transcript_path` and `last_assistant_message`; `StopFailure` covers API/auth/rate-limit/model failures and includes `error`, optional `error_details`, and optional `last_assistant_message`.
- Claude Code settings reference: durable settings live in user/project/local/managed files; `--settings` can provide per-session settings, and hooks/permissions are reloadable settings. The runner should use temporary inline/file settings and avoid editing durable settings files.
- Upstream `claude-p` README/SPEC/REPORT: useful mechanics are PTY launch, stateless DEC/XTerm probe response, `SessionStart` and `Stop` hooks via inline `--settings`, FIFO hook relay, transcript JSONL parsing, retry for transcript flush race, and SIGTERM-to-SIGKILL cleanup. Agent Bridge should port only the needed mechanics, not the CLI compatibility package.
- Lanes billing split article: the architecture point is real PTY + login shell + official interactive `claude`, not Agent SDK/print mode.

Implementation implications:

- Treat native `claude -p` as out of scope, not as a fallback.
- Treat upstream `claude-p` as reference code, not a runtime dependency.
- Add StopFailure handling to avoid misclassifying API/auth failures as generic malformed output.
- Keep hook commands silent except for the runner-owned relay channel, so hook stdout does not accidentally become Claude context.
- Test transcript flush races and fallback to `last_assistant_message`.
