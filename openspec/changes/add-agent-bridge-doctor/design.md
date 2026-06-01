## Context

The current runtime already has most of the raw signals needed for setup diagnosis:

- `providers_check` can run provider `--version` checks and optional smoke probes with timing and diagnostics.
- `task_preview` exposes provider command construction, cwd validation, launch strategy, and environment keys.
- Workspace confinement is driven by `AGENT_BRIDGE_WORKSPACES`.
- State is rooted at `AGENT_BRIDGE_STATE_DIR` or the default `~/.agent-bridge-mcp/state`.
- Claude host-runner behavior is explicit through `AGENT_BRIDGE_CLAUDE_HOST_SOCKET` and the host-runner ping protocol.

The missing piece is an operator entry point that collects cheap checks and turns them into a coherent report.

## Decisions

### Decision 1: Add `doctor` as an MCP tool

`doctor` will be listed in `tools/list` and called through `tools/call` like the existing tools. It will accept:

- `smoke` boolean, default `false`, passed through to provider readiness checks when requested.
- `providers` optional list, using the same provider names and deduplication semantics as `providers_check`.
- `cwd` optional path, used only to validate workspace/cwd policy and preview recommendations.

The response will include:

- `summary`: `ok`, `warnings`, `errors`, and a stable `status` string (`ok`, `warning`, or `error`).
- `server`: server name/version/protocol and selected important environment configuration presence.
- `workspace`: configured roots, whether roots are present/canonicalizable, and whether `cwd` is inside the policy when provided.
- `state`: resolved state dir, whether it exists or can be created/read, and registry-file status.
- `providers`: provider readiness output equivalent to `providers_check` for selected providers.
- `claudeHostRunner`: configured socket path status, launch strategy implication, and ping result when the socket is configured and reachable.
- `recommendations`: ordered operator actions, such as set `AGENT_BRIDGE_WORKSPACES`, install a provider CLI, start/restart the host runner, run `providers_check` with smoke, or inspect state-dir permissions.

### Decision 2: Default checks are cheap and deterministic

`doctor` will not spawn delegated tasks or run live provider smoke probes unless `smoke: true` is passed. By default it runs version checks and filesystem/config checks only. This preserves the default test suite's fake-provider determinism and avoids accidental model usage.

Doctor output must report environment configuration by presence, resolved path, or redacted value only. It must never echo token, API key, OAuth, auth, or password values from the process environment.

### Decision 3: Reuse existing validation and diagnostics

Provider selection, provider timing budgets, process diagnostics, and workspace validation should reuse existing code paths where practical rather than creating a separate interpretation layer. Doctor can repackage results and recommendations, but provider availability remains owned by provider readiness logic.

### Decision 4: Host-runner ping is bounded and non-task

If `AGENT_BRIDGE_CLAUDE_HOST_SOCKET` is set, doctor should attempt a host-runner ping with a 1000ms timeout and report `ok`, `unavailable`, `protocol_mismatch`, or `workspace_policy_mismatch` style diagnostics. It must not run Claude task prompts as part of the default host-runner check.

### Decision 5: Status severity mapping is explicit

Doctor should compute `summary.status` from section statuses:

| Section issue | Summary level |
| --- | --- |
| Missing or invalid `AGENT_BRIDGE_WORKSPACES` | `error` |
| Provided `cwd` cannot be canonicalized or is outside workspace policy | `error` |
| State directory cannot be created/read or registry cannot be parsed | `error` |
| Configured host-runner socket is unreachable or reports policy mismatch | `error` |
| Provider binary unavailable in version checks | `warning` |
| Host runner not configured | `warning` only when Claude is selected/configured for host-runner-sensitive diagnosis; otherwise informational |
| Smoke probe timeout when `smoke: true` | `warning` unless every selected provider fails |

Recommendations should be ordered by the same severity: workspace/state blockers first, host-runner setup next, provider installation/readiness next, optional follow-up checks last.

## Risks

- Doctor could become a parallel readiness implementation. Keep it as an aggregator over existing checks wherever possible.
- Recommendations could overclaim. Keep wording operational and avoid saying work is verified.
- Host-runner ping may need small protocol helper refactoring. Keep that helper bounded and reusable by tests.

## Rollout

This is additive. Existing clients can ignore `doctor`; operators and future prompts can use it before deeper troubleshooting.
