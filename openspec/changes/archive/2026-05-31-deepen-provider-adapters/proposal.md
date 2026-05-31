## Why

Provider-specific behavior is spread across validation, JSON schema construction, command building, environment policy, provider checks, and tests inside `src/server.mjs`. That makes adding or changing a provider risky because one conceptual change currently requires coordinated edits across unrelated sections of a 1,300+ line module.

## What Changes

- Introduce a provider adapter boundary that owns each provider's capabilities, supported options, command construction, smoke command construction, and environment policy.
- Keep the public MCP tool surface unchanged: `providers_list`, `providers_check`, `task_preview`, `task_spawn`, and task lifecycle tools continue to accept and return the same shapes unless a test exposes an existing inconsistency.
- Move provider-specific conditionals out of general task validation and task management where practical.
- Add focused adapter tests that preserve current command arguments, environment allowlists, mode validation, provider checks, and preview redaction behavior.
- Document the provider adapter contract so future providers can be added without reading the entire task manager.
- No new runtime dependencies.
- No breaking changes.

## Capabilities

### New Capabilities
- `provider-adapter-contract`: Specifies the stable contract between the provider-neutral task lifecycle and provider-specific adapter behavior.

### Modified Capabilities
- None.

## Impact

- Affected code: `src/server.mjs` initially, with likely extraction into small internal modules under `src/`.
- Affected tests: `test/server.test.mjs` may be split or augmented with provider-adapter focused tests.
- Affected documentation: `README.md` only if the provider extension model needs a short maintainer note.
- Public API impact: none intended; this is a maintainability refactor around existing provider behavior.
