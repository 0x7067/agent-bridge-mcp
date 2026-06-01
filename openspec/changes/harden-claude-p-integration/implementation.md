## Claude Provider Reliability Contract

Verified local tools:

- `claude-p 0.1.0` is installed at `/Users/pedro/.local/share/mise/installs/node/24.13.0/bin/claude-p`.
- `claude-p --help` documents `--input-file <path>`, stdin prompt input, PTY startup, Stop hook result capture, `--cwd`, `--timeout`, and `--output-format`.
- `claude 2.1.159` is installed at `/Users/pedro/.local/bin/claude`.
- `claude --help` documents native print mode with `-p/--print`, `--output-format json`, and stdin input formats.

Current bridge behavior:

- `CLAUDE_P_BIN` explicitly selects the `claude-p` path.
- `CLAUDE_BIN` selects native `claude -p` only when `CLAUDE_P_BIN` is not set.
- The Claude task path keeps the constant `/bin/zsh -lc` initialization wrapper and passes dynamic values through positional `exec "$@"` args. Provider paths, cwd paths, and prompt data are not interpolated into shell source text.
- Claude prompts are transported through child stdin for both `claude-p` and native `claude -p`; rendered prompt text is not placed in provider argv.
- Runtime prompt-transport capability is an explicit adapter table: the two supported Claude command kinds, `claude-p` and `native-claude`, are treated as stdin-capable based on verified local help/README behavior. Unknown Claude command kinds are not introduced by this change.
- `providers_check` without smoke reports binary presence only and keeps `startupVerified: false`.
- `providers_check(smoke: true)` uses the same adapter-owned Claude task command path, stdin prompt transport, shell initialization, timeout behavior, and output parsing expectations as real Claude tasks.
- Failed Claude smoke checks and failed Claude task results include bounded diagnostics with stable failure categories: `provider_timeout`, `provider_start_error`, `provider_exit_error`, and `provider_output_error`.
- Diagnostic excerpts are capped at 2048 bytes and redact rendered prompt content, original prompt content, and long prompt tokens.
- Failed `claude-p` smoke diagnostics recommend native `claude -p` when `CLAUDE_BIN` is configured, but the bridge does not silently change provider selection.

Accepted compatibility:

- Public MCP tool names and task lifecycle response fields remain compatible. Diagnostics are additive.
- `task_preview` preserves prompt redaction and now reports `"stdin": "<prompt redacted>"` for Claude prompt transport.
