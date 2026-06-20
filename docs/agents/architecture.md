# Architecture

Provider-neutral delegation runtime. The default path is an ACP router that
handles one prompt turn per session. The compatibility path is a minimal MCP
adapter with two tools for hosts that cannot launch ACP agents directly.

## At a Glance

- `agent-bridge-mcp` with no subcommand is the ACP router runtime.
- `agent-bridge-mcp mcp-adapter` is the MCP compatibility surface.
- Three conceptual layers: protocol (`mcp.rs`), ACP router (`router_runtime.rs`),
  and internal task lifecycle (`task.rs` + `provider.rs`).
- Finalized router turns emit a server-to-client JSON-RPC notification with a
  compact action summary.
- Claude is special: it runs through an owned PTY host runner bridged by Unix socket.
- All other providers are straight fork/exec with filtered env and capped IO.

## Public Protocol Surface

Agent Bridge is ACP-router-first.

- Default command: `agent-bridge-mcp`
- Default protocol: newline-delimited ACP-flavored JSON-RPC
- Supported default methods: `initialize`, `session/new`, `session/prompt`
- Terminal result kinds: `answer`, `blocker`, `failure`
- Verification status: always `not_verified`

The MCP surface is a small adapter for clients that cannot launch ACP agents directly:

- Command: `agent-bridge-mcp mcp-adapter`
- Tools: `agent_delegate`, `agent_evidence`

The task manager, provider adapters, transcript capture, worktree isolation, and result section readers remain internal implementation details.

## ACP Router Runtime

`agent-bridge-mcp` with no subcommand runs a newline-delimited JSON-RPC runtime
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

## Completion Quality

Task completion classifies provider exits in `task/complete.rs` and stores
operator-readable diagnostics with stable `failureCategory` strings. Codex
sandbox or approval denial is fatal even when the process exits 0.

Provider output validation is adapter-owned. Each provider declares
`acceptance_report` and `acceptance_criteria`; broad validation is gated by
`strict_validation = true` in `~/.agent-bridge-mcp/config.toml` or by
`AGENT_BRIDGE_STRICT_VALIDATION=true`. The default is false, preserving existing
exit-0 behavior while the validator bakes in. Claude validates the legacy
non-empty JSON `result` line; Codex checks denial text; Cursor, Kimi, and
Antigravity are currently permissive and document that validation gap.

Task spawn input may include `retryPolicy` with `maxRetries` and `backoffMs`.
Retries are actor-owned and apply only to transient failure categories:
`provider_timeout`, `provider_start_error`, and `host_runner_unavailable`.
Each retry is a new task record linked to the original by `parentAgentId`.

If a task finalizes without a final provider result but transcript events exist,
the result reader can expose `partialResults` and `next` suggests continuation or
a rerun. Partial output remains evidence, not proof of completed work.

## Module Map (`crates/agent-bridge-mcp/src/`)

| File | Responsibility | Size Hint |
|------|----------------|-----------|
| `main.rs`, `bin/agent-bridge-mcp-rs.rs` | Binary entrypoints | Tiny |
| `lib.rs` | Crate root re-export wiring | Tiny |
| `mcp.rs` | JSON-RPC 2.0 stdio framing | Small |
| `mcp_adapter.rs` | Minimal MCP adapter: `initialize`, `tools/list`, `tools/call`, `agent_delegate`, `agent_evidence` | Medium |
| `router_runtime.rs` | ACP stdio loop, session tracking, routed prompt execution | Medium |
| `server.rs` + `server/diagnostics.rs` | Internal provider readiness diagnostics | Large |
| `provider.rs` | Provider definitions, capabilities, command builders | Large |
| `router.rs` | ACP router policy, routed attempt input, terminal classification | Small |
| `task.rs` | Facade, `TaskManagerHandle`, `TaskActor` mailbox | Medium |
| `task/input.rs` | Task spawn input structs | Small |
| `task/spawn.rs` | Arg validation, worktree creation, process launch | Medium |
| `task/supervision.rs` | PID registry, process groups, signal/IO drainer | Medium |
| `task/complete.rs` | Exit classification, host-response ingestion, git snapshots | Medium |
| `task/review.rs` | Payload shaping, completion summaries, progress, `next` actions, querying | Large |
| `task/registry.rs` | Atomic registry load/save, legacy normalization | Small |
| `runtime.rs` | CLI parsing, stdio loop dispatch, panic hook, shutdown signals | Medium |
| `domain.rs` | Core enums/types: `ProviderKind`, `TaskStatus`, `FailureCategory` | Small |
| `claude_host.rs` | Unix socket server, host protocol v2 | Medium |
| `claude_interactive.rs` | Submodule facade | Tiny |
| `claude_interactive/(pty,terminal,runner,transcript,hooks,failure,setup)` | PTY engine | Complex |

## Boundaries

- `mcp` ↔ `router_runtime`/`mcp_adapter`: protocol framing stays in `mcp`;
  ACP semantics live in `router_runtime`, MCP adapter semantics live in `mcp_adapter`.
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
