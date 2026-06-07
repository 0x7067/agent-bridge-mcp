## Why
Agent Bridge's default registry can contain valid legacy records written before the public lifecycle was renamed from `taskId` to `agentId`. The current Rust task manager deserializes persisted records directly into the new `agentId` shape, so lifecycle tools fail before launch with `missing field agentId` even though the registry is valid and inspectable.

## What
- Restore persisted-state compatibility for legacy registry records by normalizing old `taskId`/`taskDir` fields to the current `agentId`/`agentDir` record shape during registry load.
- Keep the public MCP lifecycle strict: `agent_*` tools still accept and return `agentId`, and `taskId` remains an unknown public argument.
- Make `doctor` validate the registry with the same typed compatibility path used by lifecycle startup so it catches real registry load failures.
- Add regression coverage for legacy registry load, public output shape, and doctor state validation.

## Impact
- Affected code: `crates/agent-bridge-mcp/src/task.rs`, `crates/agent-bridge-mcp/src/server.rs`, and focused tests.
- Affected APIs: no public input compatibility alias is added; existing legacy records may appear as historical `agentId` values after read-time normalization.
- Affected docs/specs: `rust-single-binary-mcp` state compatibility requirements.
