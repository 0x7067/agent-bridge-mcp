## Why

Agent Bridge has reliable delegation primitives, but its public product contract
still asks callers to orchestrate provider lifecycle state with `agent_spawn`,
`agent_observe`, and `agent_result`. That is useful as a compatibility and
evidence surface, but it is not the desired native collaboration model.

The replacement direction is an ACP router: a client sends one prompt turn, the
router chooses Codex or Claude by policy and readiness, streams provider
internals only as bounded evidence/debug updates, and returns one provider-authored
answer, blocker, or classified failure.

## What Changes

- Add an ACP-router runtime path beside the existing MCP compatibility server.
- Model one routed prompt turn as the router's public contract, not caller-managed
  task lifecycle.
- Route only between Codex and Claude in v1.
- Reuse the existing task manager, provider adapters, workspace validation,
  worktree isolation, transcript capture, and diagnostics internally.
- Classify provider attempts before fallback: trusted finality, failover-eligible
  infrastructure/readiness/lifecycle failure, blocker, or terminal failure.
- Allow automatic failover only before trusted finality and only for eligible
  infrastructure, readiness, or lifecycle failures.
- Keep refusal, cancellation, auth, billing, and completed answers from silently
  falling through to another provider.

## Capabilities

### New Capabilities

- `acp-router-contract`: ACP-facing routed prompt-turn contract and runtime
  compatibility boundary.
- `provider-failover-policy`: Provider attempt classification and visible
  bounded failover rules.

### Modified Capabilities

- `provider-readiness-contract`: Router policy consumes bounded provider
  readiness instead of treating static provider presence as launchability.
- `provider-adapter-contract`: Router v1 uses the existing Codex and Claude
  adapter paths and does not add arbitrary provider discovery.
- `task-run-transcripts`: Router evidence references continue to point at
  transcript and diagnostic artifacts produced by the task path.
- `rust-single-binary-mcp`: The existing MCP runtime remains a migration
  compatibility surface while the ACP router is proven.

## Impact

- Crate: `agent-bridge-mcp` runtime, router/domain code, task review helpers,
  ACP child handling, and stdio tests.
- Dependencies: no new crates expected.
- Public API: new ACP router runtime path; no ninth MCP tool.
- Compatibility: existing MCP behavior stays unchanged during migration, but the
  replacement product contract is ACP-router-first.
