## ADDED Requirements

### Requirement: Provider adapters own launch-profile behavior
The system SHALL keep launch-profile command construction, prompt rendering, environment policy, and reduced-configuration behavior inside provider adapters.

#### Scenario: Adapter builds profile command
- **WHEN** a caller previews or spawns a task with a launch profile
- **THEN** the selected provider adapter builds the command descriptor, prompt transport, environment, and profile diagnostics for that profile.

#### Scenario: No provider skill dependency
- **WHEN** a provider adapter implements a launch profile
- **THEN** it does not read repo-owned provider skills or require provider skill files to construct or run the task.

### Requirement: Provider adapters report reduced-configuration capabilities
The system SHALL expose provider-specific reduced-configuration support through adapter-owned capability metadata.

#### Scenario: Capabilities include reduction support
- **WHEN** a caller invokes `providers_list`
- **THEN** each provider reports whether compact prompts, custom system prompts, hook disabling, skill disabling, config isolation, memory/session disabling, and environment minimization are supported, unsupported, or best-effort.

#### Scenario: Preview reports applied reductions
- **WHEN** a caller invokes `task_preview` for a reduced launch profile
- **THEN** the preview reports which reductions would be applied and which remain unsupported or best-effort.
