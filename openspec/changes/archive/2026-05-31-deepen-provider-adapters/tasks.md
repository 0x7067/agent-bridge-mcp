## 1. Pin Current Provider Behavior

- [x] 1.1 Add focused tests for provider capability metadata exposed through `providers_list`.
- [x] 1.2 Add focused tests for provider-specific option validation, including Claude `effort` and Kimi/Codex `thinking`.
- [x] 1.3 Add focused tests that preserve current command descriptors for Claude, Cursor, Kimi, and Codex.
- [x] 1.4 Add focused tests that preserve provider environment policy, including Claude `ANTHROPIC_BASE_URL` removal.
- [x] 1.5 Add focused tests that provider smoke commands use the same binary resolution path as normal task commands.

## 2. Extract Provider Adapter Boundary

- [x] 2.1 Create internal provider adapter modules or a provider registry module under `src/` without changing public exports.
- [x] 2.2 Move provider metadata, modes, supported provider options, and capability reporting behind the registry.
- [x] 2.3 Move provider command builders behind adapter-owned `command()` functions while preserving the `buildTaskCommand()` export.
- [x] 2.4 Move provider environment policy behind adapter-owned `env()` or registry-owned environment helpers while preserving `buildProviderEnv()` behavior.
- [x] 2.5 Move provider version and smoke command resolution behind adapter-owned helpers.

## 3. Rewire Existing Callers

- [x] 3.1 Update task spawn validation to ask the provider registry for mode and option support.
- [x] 3.2 Update `providers_list` to return provider capabilities from the registry.
- [x] 3.3 Update `tools/list` schema construction to derive provider and mode enums from the provider registry, or add a focused parity test if static schema construction remains clearer.
- [x] 3.4 Update `providers_check` to use adapter-owned version and smoke commands.
- [x] 3.5 Update `TaskManager.preview()` and `TaskManager.spawn()` to use the registry-backed command path.

## 4. Maintainability Cleanup

- [x] 4.1 Keep task lifecycle, log handling, registry persistence, git snapshots, and stdio transport behavior unchanged.
- [x] 4.2 Remove provider-specific switch logic from `TaskManager` where the adapter boundary now owns it.
- [x] 4.3 Add or update a short maintainer note only if the new provider adapter contract is not obvious from module names and tests.
- [x] 4.4 Check for stale references to old provider structure in README or local planning docs.
- [x] 4.5 Search `TaskManager` for residual provider-name conditionals and keep only lifecycle-owned references.

## 5. Verification

- [x] 5.1 Run `rtk npm test` and confirm all tests pass.
- [x] 5.2 Run a lightweight direct MCP smoke check for `initialize`, `tools/list`, `providers_list`, and `task_preview`.
- [x] 5.3 Run `rtk openspec status --change "deepen-provider-adapters"` and confirm the change remains apply-ready.
