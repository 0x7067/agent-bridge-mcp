## Why
Agent Bridge currently supports Claude, Cursor, Kimi/Pi, and Codex as first-class provider agents. Google Antigravity CLI exposes a local `agy` binary with non-interactive `--print` mode, so callers should be able to delegate bounded work to Antigravity through the same `providers_list`, `providers_check`, `agent_preview`, and `agent_spawn` lifecycle.

## What
- Add `antigravity` as a first-class provider backed by the `agy` CLI.
- Expose Antigravity capabilities, launch profiles, readiness checks, preview behavior, environment policy, and task execution through the existing provider adapter contract.
- Use verified CLI behavior: `agy --version` for version checks and `agy --print <prompt> --print-timeout <duration>` for non-interactive task and smoke execution.
- Report startup readiness honestly when `agy --print` requires authentication or fails to produce the smoke token.
- Update docs, tests, and specs so Antigravity appears consistently beside existing providers.

## Impact
- Affected code: `crates/agent-bridge-mcp/src/domain.rs`, `provider.rs`, `server.rs`, `tools.rs`, guidance/docs as needed, and provider lifecycle tests.
- Affected APIs: additive provider enum value `antigravity`; existing tool names and argument shapes remain unchanged.
- Affected docs/specs: provider adapter contract, provider readiness contract, task launch profiles, README provider table, and runtime guidance.
