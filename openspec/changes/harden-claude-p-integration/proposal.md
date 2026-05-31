## Why

The Claude provider is unreliable because the default path depends on `claude-p`, which wraps interactive Claude Code behavior behind a CLI/PTY bridge. Upstream `claude-p` explicitly depends on Claude Code terminal behavior and Stop hook output, so bridge failures can look like hangs, empty results, malformed JSON, or false-positive provider availability.

This matters now because the bridge already exposes Claude as a first-class provider and the Rust single-binary plan must preserve provider behavior. We need a separate reliability track that can diagnose the current Node adapter, define the intended Claude provider contract, and carry those expectations into the Rust port.

## What Changes

- Add a Claude-provider reliability capability that distinguishes binary presence from startup readiness, task execution readiness, and actionable diagnostics.
- Add deterministic test coverage for `claude-p` failure modes: hung startup, terminal/probe noise, missing Stop hook output, malformed output, non-zero exit, timeout, and prompt transport edge cases.
- Harden Claude provider command construction and health checks so `providers_check(smoke: true)` exercises the same invocation path as real Claude tasks and reports useful failure categories without leaking secrets or prompts through diagnostics or process argv.
- Define an explicit fallback and recommendation policy for native `claude -p` when `claude-p` is missing or unhealthy.
- Document troubleshooting steps for Claude reliability, including `CLAUDE_P_BIN`, `CLAUDE_BIN`, smoke checks, timeout behavior, and known upstream `claude-p` fragility.
- No public MCP tool names or task lifecycle APIs are intentionally changed.

## Capabilities

### New Capabilities
- `claude-provider-reliability`: Defines Claude provider health checks, diagnostics, command selection, prompt transport, and troubleshooting expectations for `claude-p` and native `claude -p`.

### Modified Capabilities
- `provider-adapter-contract`: Clarify that provider adapters must preserve public task behavior while allowing internal provider command transport changes required for safety and reliability.

## Impact

- Affected code: `src/provider-registry.mjs`, `src/server.mjs`, task process/result handling if diagnostics need to be surfaced from child-process failures.
- Affected tests: `test/provider-registry.test.mjs`, `test/server.test.mjs`, and new fixture scripts for deterministic fake `claude-p` behavior.
- Affected docs: `README.md` provider setup and troubleshooting sections.
- Runtime dependencies remain external: `claude-p` or native `claude` must still be installed separately. The bridge must not vendor Claude Code or `claude-p`.
- Compatibility with the active Rust migration: the Rust implementation must preserve the final Claude reliability contract defined by this change.
