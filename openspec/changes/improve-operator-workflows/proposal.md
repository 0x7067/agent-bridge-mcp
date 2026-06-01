## Why

Agent Bridge can now spawn, monitor, and inspect delegated provider tasks, but operators still need to manually synthesize final results, host-runner lifecycle steps, and real-world dogfood workflows from several outputs and README sections. The next step is to make delegated work easier to review and recover without expanding live-provider CI or weakening the main caller's responsibility for verification.

## What Changes

- Add a derived review packet to final task results so callers can quickly see status, changed files, git cleanliness, exit/error metadata, truncation flags, and recommended next actions.
- Extend MCP guidance prompts/resources with host-runner lifecycle guidance and reproducible dogfood workflows for read-only review, isolated implementation, stalled-task recovery, and provider comparison.
- Keep live provider execution opt-in and outside default CI.
- Archive completed OpenSpec changes that are already marked complete and clean up archived spec purpose text.

## Capabilities

### New Capabilities
- `delegated-review-packet`: Derived task-result summary that helps the main caller inspect delegated work before verification and cleanup.

### Modified Capabilities
- `mcp-usage-guidance`: Add discoverable operator workflows for host-runner lifecycle and dogfood delegation scenarios.
- `rust-single-binary-mcp`: Document the additive `task_result.reviewPacket` response field in the Rust MCP public result contract.

## Impact

- Affected code: task result serialization, MCP guidance prompt/resource definitions, stdio protocol tests.
- Affected API: `task_result` gains an additive `reviewPacket` object; existing fields remain unchanged.
- Affected docs: README workflow sections and OpenSpec archived specs.
- Dependencies: no new third-party dependency is expected.
