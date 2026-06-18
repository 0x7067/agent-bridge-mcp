## Why

The Claude provider repeatedly fails because Agent Bridge treats Claude as an external `claude-p`/print-mode compatibility dependency instead of owning the fragile interactive-terminal integration it relies on.

This matters now because `claude -p`/Agent SDK usage is a separate programmatic billing and reliability surface, while the intended Claude provider behavior for this bridge is a first-party local interactive Claude Code session driven through a real PTY.

## What Changes

- Replace the Claude provider's runtime policy with a bridge-owned interactive Claude runner modeled after the useful parts of `claude-p`.
- **BREAKING** Remove native `claude -p` and upstream `claude-p` as fallback provider paths for normal Claude task execution and smoke checks.
- Keep `claude` as the public provider name and preserve Agent Bridge lifecycle tools, modes, worktree behavior, redaction, diagnostics, and task result surfaces.
- Port the required `claude-p` mechanics into Rust-owned code: PTY launch, terminal probe handling, prompt injection, Stop hook/transcript capture, output formatting, timeout, and child cleanup.
- Make the host-runner path the owned Claude execution boundary so Claude runs outside Codex's sandbox while keeping structured requests, workspace validation, bounded output, and sanitized logs.
- Update provider readiness so Claude is launchable only when the owned interactive runner path is startup-verified.

## Capabilities

### New Capabilities
- `claude-interactive-provider`: Defines the owned interactive Claude provider runtime, PTY behavior, prompt/result flow, fallback policy, and parity expectations.

### Modified Capabilities
- `provider-adapter-contract`: Claude provider command resolution and readiness use the owned interactive runner, not external print-mode fallbacks.
- `claude-host-runner`: Host runner executes structured owned interactive Claude requests instead of structured `claude-p` requests.
- `claude-provider-reliability`: Claude diagnostics, readiness, and troubleshooting describe the owned runner and explicitly exclude native print-mode fallback.
- `agent-bridge-doctor`: Doctor reports the owned Claude host-runner setup and no longer describes an unconfigured host runner as a normal direct Claude launch path.
- `rust-single-binary-mcp`: External provider dependency guidance names the official interactive `claude` binary and removes `claude-p`.
- `task-launch-profiles`: Claude bare profile reports runner-owned automation hooks as required, not disabled.
- `provider-readiness-contract`: Claude owned-runner smoke checks get an interactive-runner budget and explicit smoke-token contract.
- `mcp-usage-guidance`: User-facing guidance describes the owned Claude host-runner lifecycle and removes print-mode fallback guidance.
- `agent-bridge-self-guidance`: Initialization/self-guidance names owned Claude host-runner readiness without implying direct print-mode fallback.

## Impact

- Affected code: Claude provider adapter, host-runner protocol/runtime, task output parsing, readiness checks, provider previews, and README guidance.
- Affected tests: deterministic fake Claude runner tests, PTY/probe handling tests, host-runner protocol tests, provider readiness tests, and stdio lifecycle tests.
- Dependencies: likely add a Rust PTY crate after evaluating fit; do not add a dependency until the implementation confirms the minimum viable crate.
- Compatibility: public provider name and lifecycle APIs remain stable, but operator setup changes because production Claude execution should be run through the owned host-runner path. `CLAUDE_BIN` is the only Claude binary override and points at the official interactive `claude` binary.
