## Why

Agent Bridge currently stores stdout/stderr logs, but those logs are not a normalized task transcript and make it hard to analyze provider behavior, detect useful partial results, or compare providers. The recent dogfooding runs also showed a need to call providers with a reduced prompt/configuration profile so Agent Bridge can evaluate provider behavior without competing hooks, skills, or heavy caller guidance.

## What Changes

- Add first-class task transcript collection as a persisted task artifact and public inspection surface.
- Add launch profiles for provider tasks, including a `bare` profile that uses compact bridge-owned instructions and disables or bypasses provider hooks, skills, and ambient configuration where the provider supports it.
- Add provider capability metadata and diagnostics that report whether reduced prompt/configuration behavior is supported, unsupported, or best-effort for each provider.
- Add a spike phase to empirically determine which providers can be launched with reduced system prompts, isolated configuration, disabled hooks, disabled skills, or equivalent behavior.
- Keep provider-specific launch behavior inside provider adapters rather than introducing repo-owned provider skills or a second integration path.

## Capabilities

### New Capabilities
- `task-run-transcripts`: Persist and expose normalized task run transcripts derived from provider stdout/stderr and provider-specific structured output.
- `task-launch-profiles`: Support bridge-owned launch profiles such as `bridge` and `bare`, including provider-specific capability reporting for reduced prompt/configuration execution.

### Modified Capabilities
- `provider-adapter-contract`: Provider adapters report and implement launch-profile support, including best-effort reduced configuration behavior.
- `delegated-review-packet`: Review packets may reference transcript availability and detected final/partial result evidence without treating provider output as verification.

## Impact

- Affected code: provider adapters, task lifecycle/log draining, task result/reporting, public tool schemas, transcript storage, and integration tests.
- Affected API: additive `task_spawn`/`task_preview` launch-profile argument, additive task transcript inspection surface, additive provider capability metadata, and additive task result/review packet fields.
- Affected storage: new per-task transcript artifact under each task directory.
- No new provider-skill abstraction is introduced.
