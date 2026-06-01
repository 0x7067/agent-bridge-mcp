## Context

During the `add-agent-bridge-doctor` implementation, a delegated Codex task was spawned through Agent Bridge in `implement` mode with `isolation: "none"` and `cwd` set to this repository. Codex emitted a provider error on stderr:

```text
patch rejected: writing outside of the project; rejected by user approval settings
```

The task did not transition to a final failed state and remained `running` until it was manually stopped. The useful failure evidence existed only in logs, while lifecycle tools did not clearly report the task as unrecoverable.

The likely implementation surfaces are the Codex provider command builder, task child wait/finalization, log drain completion, failure classification, and `reviewPacket` guidance. The fix must be deterministic in CI with fake providers and must not require live Codex credentials or network access.

## Goals / Non-Goals

**Goals:**
- Reproduce the Codex sandbox/approval rejection with fake-provider tests before changing production behavior.
- Identify whether the root cause is provider command construction, prompt/workspace assumptions, Codex stderr behavior, or task lifecycle handling.
- Guarantee that unrecoverable Codex sandbox/approval denials transition to a final failed task within a bounded time.
- Preserve stdout MCP protocol discipline and expose actionable redacted diagnostics through lifecycle tools.
- Update guidance so operators know when to inspect logs, stop, narrow the prompt, switch isolation, or fix workspace policy.

**Non-Goals:**
- Do not add a new MCP tool.
- Do not silently relax Codex sandbox permissions.
- Do not parse arbitrary Codex prose as proof of success.
- Do not require live Codex execution in the default test suite.
- Do not change Claude, Cursor, or Kimi behavior except through shared lifecycle bug fixes proven by tests.

## Decisions

### Decision 1: Treat sandbox/approval denials as terminal provider failures

Agent Bridge should classify Codex messages that indicate an unrecoverable sandbox or user-approval denial as task failures, not as long-running progress. This can be done by detecting process exit with known stderr text, or by adding bounded early-abort handling if Codex leaves the process alive after such a fatal message.

Alternative considered: rely on callers to stop stalled tasks manually. That keeps the current failure mode and makes automation brittle.

### Decision 2: Add a Codex-specific diagnostic category

Diagnostics should include a stable failure category such as `provider_sandbox_denied` or an equivalent explicit category, plus provider name, command path/kind, launch strategy, exit metadata when available, and redacted stdout/stderr excerpts. This is more actionable than a generic timeout when the provider has already explained the denial.

Alternative considered: map every denial to `provider_exit_error`. That keeps the old coarse category but loses the distinction operators need for remediation.

### Decision 3: Keep prompt and secret redaction conservative

The diagnostic excerpt redaction path should continue to redact prompts and credential values. Tests should prove the original prompt and secret-like values do not appear in diagnostics, logs-derived excerpts, or review packets.

Alternative considered: include full stderr/stdout for easier debugging. That is not acceptable for provider prompts or credentials.

### Decision 4: Investigate Codex command shape before changing sandbox mode

The implementation should inspect whether `codex exec --cd <cwd> --sandbox <mode>` plus the generated prompt can trigger out-of-project patch attempts. If command shape is implicated, adjust the Codex adapter with focused tests. If not, keep command behavior and fix lifecycle/error detection only.

Alternative considered: immediately switch implementation mode to a looser Codex sandbox. That may mask the symptom while weakening safety.

### Decision 5: Use stdio fake-provider regression tests

Default verification should simulate Codex stderr and lifecycle behavior with fake providers. Tests should cover immediate non-zero exit, fatal stderr followed by a hung process, and normal successful Codex-like output. Live Codex dogfood remains optional.

Alternative considered: make a live Codex regression part of CI. That would depend on credentials, network/model behavior, and local Codex policy.

## Risks / Trade-offs

- Over-specific stderr matching could miss future Codex wording changes. Mitigation: match a small set of stable concepts such as `patch rejected`, `outside of the project`, `approval`, and `sandbox`, and keep generic timeout fallback.
- Early-abort logic could terminate a provider that would have recovered. Mitigation: scope early-abort to Codex plus explicit fatal denial patterns.
- Adding or changing diagnostic categories or `errorType` values could surprise callers that key on current values. Mitigation: document intentional breaks in the spec and tests, and prefer the clearest contract over compatibility with stale categories.
- Investigation may find the task was launched with stale installed binary/config rather than current source. Mitigation: tasks include smoke testing the installed MCP command and documenting the operational restart path.
