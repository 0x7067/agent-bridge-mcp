## 1. Public Tool Contract

- [ ] 1.1 Add failing protocol and stdio tests proving `tools/list` includes `doctor` with strict input schema.
- [ ] 1.2 Add `Doctor` to `ToolName`, tool definitions, unknown-argument allowlist, and tool-name rendering.

## 2. Doctor Core Report

- [ ] 2.1 Add failing stdio tests for default `doctor` output sections: `summary`, `server`, `workspace`, `state`, `providers`, `claudeHostRunner`, and `recommendations`.
- [ ] 2.2 Implement doctor report assembly as a focused module or focused server helper that does not spawn delegated tasks by default.
- [ ] 2.3 Add failing stdio test proving default `doctor` does not create task records, execute task prompts, or invoke provider task modes.
- [ ] 2.4 Add failing stdio test proving token/API key/OAuth/auth/password environment values are represented only by presence or redacted markers and raw values are not present in doctor output.
- [ ] 2.5 Include server name/version/protocol metadata and relevant environment-presence metadata without exposing secret values.
- [ ] 2.6 Add failing tests proving `summary.status` becomes `error` for workspace/state blockers and `warning` for provider-binary readiness concerns.

## 3. Workspace And State Diagnostics

- [ ] 3.1 Add failing tests for missing `AGENT_BRIDGE_WORKSPACES`, invalid workspace roots, `cwd` outside workspace policy, and non-canonicalizable `cwd`.
- [ ] 3.2 Add failing tests for usable and unusable `AGENT_BRIDGE_STATE_DIR` behavior, including registry parse failure.
- [ ] 3.3 Add failing test proving recommendations order workspace/state blockers before host-runner and provider follow-ups.
- [ ] 3.4 Implement workspace and state-dir diagnostics with actionable status and recommendations.

## 4. Provider Readiness Integration

- [ ] 4.1 Add failing tests proving default `doctor` uses provider version checks without smoke.
- [ ] 4.2 Add failing tests proving `doctor` accepts `smoke`, `providers`, `aggregateTimeoutMs`, `providerTimeoutMs`, and `cwd`, with provider controls matching `providers_check` and `cwd` using workspace policy validation.
- [ ] 4.3 Add failing test proving duplicate providers are deduplicated before checks run.
- [ ] 4.4 Reuse existing provider readiness execution for doctor provider output.

## 5. Claude Host-Runner Diagnostics

- [ ] 5.1 Add failing tests for host-runner not configured, configured but unavailable within 1000ms, configured reachable ping, and protocol mismatch.
- [ ] 5.2 Add failing test for workspace-policy mismatch recommendation if the host-runner ping reports mismatch.
- [ ] 5.3 Implement bounded host-runner ping diagnostics without running Claude task prompts.

## 6. Guidance And Docs

- [ ] 6.1 Add failing guidance/protocol tests proving caller workflow and host-runner guidance mention `doctor`.
- [ ] 6.2 Update MCP guidance resources/prompts to recommend `doctor` as the first setup troubleshooting step.
- [ ] 6.3 Update README with `doctor` usage examples and expected report interpretation.

## 7. Verification

- [ ] 7.1 Run `cargo fmt --check`.
- [ ] 7.2 Run focused protocol and stdio doctor tests.
- [ ] 7.3 Run full `cargo test`.
- [ ] 7.4 Run `cargo clippy --all-targets -- -D warnings`.
- [ ] 7.5 Run `openspec validate add-agent-bridge-doctor`.
