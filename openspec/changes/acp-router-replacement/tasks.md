## 1. Contract

- [x] 1.1 Add `acp-router-contract` spec scenarios for ACP initialize, session creation, prompt finality, compact results, and evidence references.
- [x] 1.2 Add `provider-failover-policy` spec scenarios for eligible failover, blockers, trusted finality, and visible diagnostics.
- [ ] 1.3 Validate the change with `openspec validate acp-router-replacement --strict` when the CLI is available.

## 2. Router Domain and Policy

- [x] 2.1 Add a small `router` module and crate export.
- [x] 2.2 Define routed-turn input, candidate policy, attempt outcome, terminal result, and disposition types.
- [x] 2.3 Restrict router candidates to `codex` and `claude`.
- [x] 2.4 Map task/provider evidence into `TrustedFinal`, `FailoverEligible`, `Blocker`, or `TerminalFailure`.
- [x] 2.5 Add pure unit tests for finality, blockers, and failover eligibility.

## 3. Internal Routed-Turn Execution

- [ ] 3.1 Execute router attempts through `TaskManagerHandle` using existing spawn, wait, and result paths.
- [ ] 3.2 Preserve router-requested workspace confinement and worktree isolation through existing task arguments.
- [ ] 3.3 Promote ACP `stopReason` into task diagnostics so refusal and cancellation classify without transcript text parsing.
- [ ] 3.4 Return compact attempt evidence references without embedding raw stdout, stderr, transcript, or diff bodies.

## 4. ACP Router Runtime

- [ ] 4.1 Add an explicit `agent-bridge-mcp acp-router` runtime path without changing default MCP behavior.
- [ ] 4.2 Handle ACP `initialize`, `session/new`, and `session/prompt` over newline-delimited JSON-RPC.
- [ ] 4.3 Emit bounded `session/update` evidence/debug events for provider internals.
- [ ] 4.4 Return one final answer, blocker, or classified failure for each prompt turn.
- [ ] 4.5 Add stdio tests proving MCP default behavior is unchanged and ACP router stdout stays valid JSON-RPC.

## 5. Diagnostics, Docs, and Verification

- [ ] 5.1 Add compact router result diagnostics with provider, terminal kind, attempts, failover trail, and evidence refs.
- [ ] 5.2 Add fake-provider tests for routed success, infrastructure failover, refusal, cancellation, auth/billing blockers, and retained evidence.
- [ ] 5.3 Update docs and guidance to describe ACP router replacement and MCP lifecycle migration compatibility.
- [ ] 5.4 Run `cargo test -p agent-bridge-mcp -- --test-threads=1`.
- [ ] 5.5 Run `scripts/quality.sh`.
