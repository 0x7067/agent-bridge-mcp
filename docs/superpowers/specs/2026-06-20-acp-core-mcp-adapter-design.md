# ACP Core With Minimal MCP Adapter Design

## Goal

Make Agent Bridge an ACP-first router while preserving a small path for Codex
and Claude Code to use it before they can consume ACP agents directly.

The public product contract becomes one routed prompt turn:

- client creates or reuses a router session
- client sends one prompt
- Agent Bridge chooses a provider, owns waiting and bounded failover
- client receives one terminal `answer`, `blocker`, or `failure`
- raw evidence remains available by reference for audit and debugging

## Decision

Remove the old public MCP lifecycle product. Do not keep
`agent_spawn`, `agent_observe`, `agent_result`, `agent_list`, `agent_stop`, or
`agent_remove` as the compatibility contract.

Keep MCP only as a thin adapter for hosts that currently integrate tools through
MCP, especially Codex and Claude Code. The adapter exposes the router contract,
not the old lifecycle choreography.

## Non-Goals

- No legacy eight-tool MCP compatibility.
- No long-lived MCP migration mode.
- No provider synthesis or semantic second opinion.
- No rewrite of the task manager, provider adapters, transcript capture, or
  worktree isolation in this change.
- No rename of the crate or installed binary in the first implementation pass.

## Research Summary

Codex and Claude Code currently document MCP as their external tool integration
surface. Both also support subagent workflows, but those subagents are native
to each host and are not documented as generic ACP clients.

ACP is the right direct protocol for ACP-capable editors and agent clients. For
Codex and Claude Code, a tiny MCP adapter is the practical bridge until those
hosts can launch or connect to arbitrary ACP agents.

## Architecture

```text
ACP clients
  -> agent-bridge-mcp default stdio runtime
  -> ACP router
  -> task manager / provider adapters / evidence store

Codex or Claude Code
  -> agent-bridge-mcp mcp-adapter MCP server
  -> ACP router path in-process
  -> task manager / provider adapters / evidence store
```

The important boundary is product-level, not code-level:

- ACP router is the product interface.
- MCP adapter is a protocol shim.
- Task manager remains the internal execution engine.

## Components

### ACP Router Core

`agent-bridge-mcp` should run the ACP router by default.

The router supports:

- `initialize`
- `session/new`
- `session/prompt`

`session/prompt` accepts:

- `sessionId`
- `prompt`
- `mode`
- `cwd`
- `timeoutSeconds`
- optional provider policy, currently Codex and Claude candidates

It returns a `routerResult`:

- `terminalKind`: `answer`, `blocker`, or `failure`
- `provider`
- `finalText`
- `failureCategory`
- `blockerReason`
- `diagnostics.attempts`
- `diagnostics.failoverTrail`
- `diagnostics.evidenceRefs`
- `verificationStatus: "not_verified"`

### Internal Engine

Keep the existing task lifecycle internally:

- provider command construction
- workspace validation
- managed worktrees
- spawn and wait
- transcript capture
- result classification
- failure diagnostics
- evidence section readers

These are reliability primitives. They should stop leaking as the public
collaboration model, but they do not need to be deleted.

### MCP Adapter

Add a minimal adapter subcommand:

```text
agent-bridge-mcp mcp-adapter
```

The adapter exposes only router-shaped MCP tools.

#### `agent_delegate`

Starts one routed prompt turn and waits for a terminal router result.

Input:

- `prompt`
- `cwd`
- `mode`
- `timeoutSeconds`
- optional `policy.candidates`

Output:

- `terminalKind`
- `provider`
- `finalText`
- `failureCategory`
- `blockerReason`
- `verificationStatus: "not_verified"`
- `diagnostics`
- `evidenceRefs`

#### `agent_evidence`

Fetches bounded evidence by reference.

Input:

- `evidenceRef`
- `sections`: `summary`, `stdout`, `stderr`, `transcript`, `diff`, or
  `changedFiles`
- pagination and byte limits matching existing internal evidence readers

Output:

- requested bounded evidence
- truncation and cursor metadata

The adapter must not expose lifecycle operations. It does not allow callers to
spawn and poll provider tasks manually.

## Removed MCP Surface

Delete or stop exporting the old MCP server behavior:

- `tools/list` advertising the eight lifecycle tools
- `tools/call` dispatch for lifecycle tools
- MCP prompt/resource guidance that teaches lifecycle orchestration
- completion notifications tied to MCP lifecycle attention
- output schemas for old lifecycle tools

Keep generic JSON-RPC structs if the ACP router still uses them. Rename
`mcp.rs` to `jsonrpc.rs` only after the public behavior is removed and tests are
green.

## Client Experience

ACP clients talk to Agent Bridge directly:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
{"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"/repo"}}
{"jsonrpc":"2.0","id":3,"method":"session/prompt","params":{"sessionId":"router-...","mode":"implement","prompt":{"type":"text","text":"Fix the failing test"},"timeoutSeconds":600}}
```

Codex and Claude Code use MCP only through the adapter:

```text
agent_delegate(prompt: "Fix the failing test", cwd: "/repo", mode: "implement")
```

Both paths produce the same conceptual result:

```json
{
  "terminalKind": "answer",
  "provider": "codex",
  "finalText": "Changed the failing assertion. Verification still belongs to the caller.",
  "verificationStatus": "not_verified",
  "evidenceRefs": []
}
```

## Error Handling

- Provider-authored final text is trusted finality for the routed prompt turn.
- Auth, billing, user cancellation, and explicit refusal return `blocker`.
- Provider start failure, timeout, closed stdout, and host-runner availability
  issues may fail over before finality.
- Failover must appear in `diagnostics.failoverTrail`.
- The router never claims project verification; every terminal result includes
  `verificationStatus: "not_verified"`.

## Testing

Use deterministic fake providers and stdio JSON-RPC tests.

Required coverage:

- default binary starts ACP router, not MCP tools
- `tools/list` is rejected by the default runtime
- `session/new` creates a router session without launching providers
- `session/prompt` returns one terminal router result
- failover happens only for eligible pre-finality infrastructure failures
- blockers do not silently fail over
- MCP adapter exposes `agent_delegate` and `agent_evidence` only
- adapter `agent_delegate` returns the same router result shape as ACP
- adapter does not expose old lifecycle tool names

## Rollout

1. Make ACP router the default runtime.
2. Remove the old MCP lifecycle server from the default binary.
3. Add the `mcp-adapter` subcommand for Codex and Claude Code.
4. Delete old MCP lifecycle docs, prompts, schemas, and tests.
5. Keep internal task/evidence primitives until the router has equivalent
   coverage and diagnostics.

## Naming And Packaging

Use the existing `agent-bridge-mcp` binary name for this pass to avoid packaging
churn. The default command runs ACP. The `mcp-adapter` subcommand exists only
for hosts that need MCP as an invocation protocol.

Rename the installed binary to `agent-bridge` only after the ACP default and
adapter split are working and documented.
