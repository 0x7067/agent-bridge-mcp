## Why

Claude Code stores interactive login state in macOS Keychain. When the MCP server runs inside Codex's sandbox, `claude-p` can be installed and reachable but still fail task execution because the sandboxed child process cannot access the host Keychain-backed Claude auth.

Live diagnosis confirmed that `claude-p` succeeds outside the sandbox, and succeeds with the bridge's same `/bin/zsh -flc` wrapper outside the sandbox, while the sandboxed MCP smoke path times out or reports unavailable auth. The bridge needs a deliberate host-side execution path for Claude only.

## What Changes

- Add an explicit host-runner path for the Claude provider so `claude-p` can execute outside the Codex sandbox and reach macOS Keychain auth.
- Keep `claude-p` as the default and non-negotiable Claude command path; do not switch to native `claude -p` as the solution.
- Keep all existing workspace, cwd, prompt redaction, timeout, stdout/stderr capture, and task lifecycle constraints around host-runner execution.
- Provide diagnostics and preview output that make it clear when Claude is configured to use a host runner.
- Preserve the existing in-process execution path for deterministic tests and non-host-runner environments.
- Do not use `clm` or Codex remote-control as part of this change.

## Capabilities

### New Capabilities
- `claude-host-runner`: Covers host-side Claude execution, runner selection, request constraints, diagnostics, and safety boundaries.

### Modified Capabilities
- `claude-provider-reliability`: Claude reliability requirements must account for macOS Keychain auth and the need to execute `claude-p` outside Codex's sandbox.
- `provider-adapter-contract`: Provider command descriptors must support adapter-owned launch strategies, not only direct child-process execution.

## Impact

- Affected code: Claude provider command construction, task spawning, provider smoke checks, diagnostics, preview output, tests, and README troubleshooting/setup docs.
- New runtime surface: a narrow local host runner for Claude provider execution.
- Security: host execution is limited to the Claude provider and must preserve existing workspace/cwd validation, prompt redaction, timeout, and log capture.
- Dependencies: no third-party package is added unless implementation proves the Rust standard library and current Tokio stack are insufficient.
