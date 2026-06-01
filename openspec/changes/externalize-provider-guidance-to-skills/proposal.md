## Why

Agent Bridge currently owns both runtime orchestration and provider-specific usage guidance. That makes the bridge harder to reason about because stable MCP lifecycle behavior sits next to fast-changing CLI runbooks for Claude, Codex, Cursor, and the Kimi-backed `pi` CLI.

Moving provider-specific usage guidance into versioned agent skills keeps Agent Bridge focused on MCP-native task orchestration while giving humans and calling agents a better place to maintain CLI-specific invocation rules, safety notes, and troubleshooting recipes.

## What Changes

- Introduce repo-owned provider guidance skills for each first-class provider: Claude, Codex, Cursor, and Kimi.
- Define a skill guidance contract that separates human/operator runbook content from runtime-owned command construction.
- Update Agent Bridge guidance and documentation to point operators at provider skills for direct CLI usage, provider-specific caveats, and manual troubleshooting.
- Keep MCP tool schemas, task lifecycle APIs, provider command builders, readiness checks, workspace policy, and result inspection owned by Agent Bridge.
- Add validation so provider skill guidance remains present, current, and aligned with supported provider names and modes.
- Public MCP tool names, response shapes, guidance resources, and provider task behavior may change when doing so creates a cleaner provider-skill/runtime boundary; intentional breaking changes must be documented and tested.

## Capabilities

### New Capabilities

- `provider-skill-guidance`: Defines the repo-owned contract for provider-specific agent skills, including required content, provider coverage, safety boundaries, and validation expectations.

### Modified Capabilities

- `mcp-usage-guidance`: Guidance SHALL distinguish skill-owned provider runbooks from bridge-owned MCP runtime workflows and point operators to the correct surface.
- `provider-adapter-contract`: Provider adapter metadata SHALL remain the runtime source of truth for supported providers, modes, and command construction while allowing documentation/skills to consume or mirror that metadata through validation.

## Impact

- Affected docs: README provider guidance, MCP guidance resources, and any setup/troubleshooting sections that currently embed CLI-specific runbooks.
- Affected specs: new provider skill guidance requirements plus deltas to MCP usage guidance and provider adapter contract.
- Affected code/tests: likely Rust guidance resources, provider metadata helpers, and tests that assert provider skill files exist and match supported provider names/modes.
- Affected local artifacts: repo-owned skill files or templates for Claude, Codex, Cursor, and Kimi. The Kimi provider skill is named `pi-agent`, documents the `pi` CLI, and pins a Kimi model in its default direct invocation. Existing personal skills under `~/.claude/skills` are examples, not the source of truth for this repo.
