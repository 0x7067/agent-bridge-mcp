# Architecture

Provider-neutral delegation runtime. The default path is an MCP compatibility
server where a caller previews, starts, observes, inspects, stops, and removes
delegated tasks while staying responsible for verification. The replacement
path is an explicit ACP router runtime that owns one prompt turn.

## At a Glance

- Eight MCP tools only on the compatibility surface — extend via options, not new tools.
- `agent-bridge-mcp acp-router` is the ACP replacement prompt-turn contract.
- Three conceptual layers: protocol (`mcp.rs`), dispatch (`server.rs`), lifecycle (`task.rs` + `provider.rs`).
- Finalized current-session agents emit a server-to-client JSON-RPC notification with a compact action summary.
- Claude is special: it runs through an owned PTY host runner bridged by Unix socket.
- All other providers are straight fork/exec with filtered env and capped IO.

## The Eight MCP Tools

`providers_list`, `doctor`, `agent_spawn`, `agent_observe`, `agent_result`,
`agent_list`, `agent_stop`, `agent_remove`. Keep this surface lean — prefer
adding options to an existing tool over adding a ninth. Tool schemas and dispatch
live in `src/tools.rs` / `src/mcp.rs`.

This lifecycle remains migration compatibility while ACP-router clients adopt
the replacement surface. It is still supported for caller-managed delegation,
but it is not the final routed-collaboration product contract.

## ACP Router Runtime

`agent-bridge-mcp acp-router` runs a separate newline-delimited JSON-RPC runtime
for ACP clients. It handles `initialize`, `session/new`, and `session/prompt`;
it does not advertise MCP tools.

A routed prompt turn asks Agent Bridge once. Router policy considers only
`codex` and `claude` candidates in v1, executes provider attempts through the
existing task manager, and returns one of three terminal outcomes:

- `answer`: provider-authored final text trusted for that prompt turn.
- `blocker`: semantic refusal, cancellation, auth, billing, or setup blocker;
  the router does not ask another provider for a second opinion.
- `failure`: classified terminal failure when no attempt can produce an answer.

Infrastructure failures such as provider timeout, provider start failure, host
runner unavailability, runner timeout, or client disconnect may fail over to the
next candidate. The final `routerResult` carries selected provider, terminal
kind, attempts, compact diagnostics, `failoverTrail`, and evidence refs. Normal
router output does not embed raw stdout, stderr, transcript, or diff bodies.

## Module Map (`crates/agent-bridge-mcp/src/`)

| File | Responsibility | Size Hint |
|------|----------------|-----------|
| `main.rs`, `bin/agent-bridge-mcp-rs.rs` | Binary entrypoints | Tiny |
| `lib.rs` | Crate root re-export wiring | Tiny |
| `mcp.rs` | JSON-RPC 2.0 stdio framing | Small |
| `tools.rs` | Eight-tool schemas + param parsing | Medium |
| `server.rs` + `server/diagnostics.rs` | Request router + `doctor` tool | Large |
| `provider.rs` | Provider definitions, capabilities, command builders | Large |
| `router.rs` | ACP router policy, routed attempt input, terminal classification | Small |
| `task.rs` | Facade, `TaskManagerHandle`, `TaskActor` mailbox | Medium |
| `task/spawn.rs` | Arg validation, worktree creation, process launch | Medium |
| `task/supervision.rs` | PID registry, process groups, signal/IO drainer | Medium |
| `task/complete.rs` | Exit classification, host-response ingestion, git snapshots | Medium |
| `task/review.rs` | Payload shaping, completion summaries, progress, `next` actions, querying | Large |
| `task/registry.rs` | Atomic registry load/save, legacy normalization | Small |
| `runtime.rs` | Stdio loop, panic hook, shutdown signals | Medium |
| `domain.rs` | Core enums/types: `ProviderKind`, `TaskStatus`, `FailureCategory` | Small |
| `guidance.rs` | Next-action text, prompts, resources | Large (text) |
| `claude_host.rs` | Unix socket server, host protocol v2 | Medium |
| `claude_interactive.rs` | Submodule facade | Tiny |
| `claude_interactive/(pty,terminal,runner,transcript,hooks,failure,setup)` | PTY engine | Complex |

## Boundaries

- `mcp` ↔ `tools`: protocol framing stays in `mcp`; tool semantics in `tools`.
- `task`/`provider` are the lifecycle core; `claude_interactive` is one provider
  implementation behind that boundary — don't leak PTY details upward.
- Domain types belong in `domain.rs`, not duplicated per module (jscpd watches this).
- One level of links only; the dependency graph is checked in CI for cycles.

## Spec-Driven Changes

Non-trivial features are tracked under `openspec/` (`schema: spec-driven`,
`changes/` + `specs/`). Check there for in-flight specs before large work.

## Going Deeper

- [System context](../ARCHITECTURE/system-context.md) — C4 Level 1: actors and external dependencies
- [Containers](../ARCHITECTURE/containers.md) — C4 Level 2: internal building blocks and comms
- [Integrations](../INTEGRATIONS.md) — provider CLI details, dependency graph
- [Backend codemap](../CODEMAPS/backend.md) — entry points, data flow, key modules
- [ADR/INDEX](../ADR/INDEX.md) — architectural decisions (tool consolidation, lifecycle hardening, decomposition, PTY host runner)
