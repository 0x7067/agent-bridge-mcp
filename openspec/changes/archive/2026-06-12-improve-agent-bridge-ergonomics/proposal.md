## Why

Agent Bridge configuration and operational interfaces are entirely environment-variable driven. Multi-root workspaces, host-runner sockets, and state directories are passed via opaque env vars with duplicated home-expansion logic scattered across the crate. Operators lack standalone CLI affordances (`--help`, `--config-check`, `--version`), and progress observation relies on wasteful 50-ms polling loops. These ergonomic gaps increase onboarding friction, complicate shared workstation setups, and burn caller context windows with redundant polling traffic.

## What Changes

- Add a layered configuration loader (`~/.agent-bridge-mcp/config.toml` or `.yaml`) with precedence: defaults < config file < env vars < CLI flags. Consolidates `AGENT_BRIDGE_WORKSPACES`, `AGENT_BRIDGE_STATE_DIR`, `AGENT_BRIDGE_CLAUDE_HOST_SOCKET`, and `AGENT_BRIDGE_MAX_ACTIVE_TASKS` into a typed `Config` struct.
- Introduce a minimal CLI surface using `clap`: `--help`, `--version`, `--config-check`, and `--doctor-smoke` for pre-flight validation without requiring an MCP client.
- Replace the tight-polling `agent_observe` loop with an optional notification/streaming path. Emit lifecycle events via MCP notifications where the client supports them; retain the existing tool-based polling as a fallback for thin clients.
- Adopt the `tracing` ecosystem with a JSON subscriber, propagating structured spans (`agent_id`, `provider`, `mode`) across the actor, launcher, and IO drainer tasks.
- Support hot reloading of workspace roots via either config-file watching or an explicit reload trigger, eliminating the need to restart the MCP server when `AGENT_BRIDGE_WORKSPACES` changes.

## Capabilities

### New Capabilities
- `config-file-loader`: Layered configuration from file, env, and CLI flags with typed validation.
- `cli-pre-flight-help`: Standalone binary affordances (`--help`, `--version`, `--config-check`, `--doctor-smoke`).
- `streaming-event-notifications`: Asynchronous lifecycle/event delivery reducing observe polling overhead.
- `structured-json-logging`: Trace propagation across the task actor and provider subprocess boundaries.
- `hot-reload-workspaces`: Runtime refresh of workspace root policies without server restart.

### Modified Capabilities
- `agent-bridge-doctor`: Extended to support CLI-triggered `--doctor-smoke` as a pre-flight convenience. Requirement delta is additive only; existing MCP tool contract remains unchanged.

## Impact

- Crate: `agent-bridge-mcp` (configuration, runtime, server, task)
- Dependencies: Adds `toml` (or `serde_yaml`) and `clap` crates; adopts `tracing` and `tracing-subscriber`
- MCP protocol: Advertises `notifications` capability when streaming is enabled; backward compatible because polling path persists
- Files affected: `runtime.rs`, `server.rs`, `task.rs`, `domain.rs`, `provider.rs`, `guidance.rs`, and new `config.rs`
