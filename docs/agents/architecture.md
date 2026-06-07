# Architecture

Provider-neutral MCP server: a caller previews, starts, observes, inspects,
stops, and removes delegated tasks while staying responsible for verification.

## The eight MCP tools

`providers_list`, `doctor`, `agent_spawn`, `agent_observe`, `agent_result`,
`agent_list`, `agent_stop`, `agent_remove`. Keep this surface lean — prefer
adding options to an existing tool over adding a ninth. Tool schemas and dispatch
live in `src/tools.rs` / `src/mcp.rs`.

## Module map (`crates/agent-bridge-mcp/src/`)

- `main.rs`, `bin/agent-bridge-mcp-rs.rs` — binary entrypoints.
- `lib.rs` — crate root wiring the modules together.
- `mcp.rs` — MCP JSON-RPC protocol layer (stdio framing, request/response).
- `tools.rs` — the eight-tool surface: schemas, argument parsing, dispatch.
- `server.rs` + `server/diagnostics.rs` — server loop and `doctor` diagnostics.
- `provider.rs` — first-class provider definitions and capabilities.
- `task.rs` + `task/supervision.rs` — task lifecycle, spawn/observe/result state, child supervision.
- `runtime.rs` — async runtime plumbing.
- `domain.rs` — core domain types shared across modules.
- `guidance.rs` — the structured guidance / next-action text returned to callers.
- `claude_host.rs` — host-runner integration for Claude.
- `claude_interactive.rs` + `claude_interactive/` — interactive PTY provider:
  `pty.rs`, `terminal.rs`, `runner.rs`, `transcript.rs`, `hooks.rs`,
  `failure.rs`, `setup.rs`.

## Boundaries

- `mcp` ↔ `tools`: protocol framing stays in `mcp`; tool semantics in `tools`.
- `task`/`provider` are the lifecycle core; `claude_interactive` is one provider
  implementation behind that boundary — don't leak PTY details upward.
- Domain types belong in `domain.rs`, not duplicated per module (jscpd watches this).
- One level of links only; the dependency graph is checked in CI for cycles.

## Spec-driven changes

Non-trivial features are tracked under `openspec/` (`schema: spec-driven`,
`changes/` + `specs/`). Check there for in-flight specs before large work.
