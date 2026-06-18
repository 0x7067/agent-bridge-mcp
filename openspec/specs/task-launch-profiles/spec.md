# task-launch-profiles Specification

## Purpose
Define task launch profiles that let callers choose between the normal Agent Bridge prompt/configuration strategy and reduced provider-specific profiles for behavior analysis, comparison, and lower-interference delegation runs.
## Requirements
### Requirement: Tasks support explicit launch profiles
The system SHALL allow callers to select a task launch profile that controls prompt wrapping and provider configuration strategy.

#### Scenario: Previewing a launch profile
- **WHEN** a caller invokes `agent_preview` with a supported launch profile
- **THEN** the response includes the selected profile, profile-specific command metadata, prompt strategy metadata, and any provider-specific reduction diagnostics.

#### Scenario: Spawning with a launch profile
- **WHEN** a caller invokes `agent_spawn` with a supported launch profile
- **THEN** the provider adapter launches the task using that profile and the task lifecycle records the selected profile.

#### Scenario: Unsupported launch profile
- **WHEN** a caller requests a launch profile unsupported by the selected provider
- **THEN** validation rejects the request before spawning a process and reports the provider/profile incompatibility.

### Requirement: Bridge profile preserves Agent Bridge task guidance
The system SHALL provide a `bridge` launch profile that uses Agent Bridge's normal task prompt and provider adapter behavior.

#### Scenario: Bridge profile task
- **WHEN** a caller launches a task with profile `bridge`
- **THEN** the provider receives the normal Agent Bridge task wrapper including mode, provider, instruction, user prompt, and final-report expectations.

### Requirement: Bare profile uses compact instructions and reduced provider configuration
The system SHALL provide a `bare` launch profile that uses compact bridge-owned instructions and disables or bypasses provider hooks, skills, session memory, context files, and ambient configuration where the provider supports it, while preserving runner-owned automation that is required for provider correctness.

#### Scenario: Bare profile prompt
- **WHEN** a caller launches a task with profile `bare`
- **THEN** the rendered task prompt is compact and contains only the minimum mode, cwd, safety, final-report, and user-instruction content required by Agent Bridge.

#### Scenario: Bare profile reductions supported
- **WHEN** the selected provider supports disabling hooks, skills, session memory, context files, or ambient configuration
- **THEN** the provider adapter applies those reductions and records them in preview and result diagnostics.

#### Scenario: Bare profile reductions best-effort
- **WHEN** the selected provider does not expose a reliable way to disable one or more ambient behavior sources
- **THEN** the provider adapter records those reductions as unsupported or best-effort rather than implying they were disabled.

#### Scenario: Bare profile preserves Claude runner hooks
- **WHEN** the selected provider is Claude and the owned interactive runner requires runner-owned hooks for prompt injection, Stop capture, or transcript relay
- **THEN** bare profile does not disable those runner-owned hooks.
- **AND** profile diagnostics report them as required Agent Bridge automation rather than ambient user hooks.

#### Scenario: Bare profile naming caveat
- **WHEN** a caller inspects bare-profile metadata
- **THEN** the response makes clear that `bare` means provider-specific reduced configuration and that the actual applied reductions are the profile diagnostics.

### Requirement: Unblocked profile uses explicit provider permission bypass
The system SHALL provide an explicit `unblocked` launch profile for providers with known ACP permission-bypass flags, while preserving Agent Bridge workspace validation before command construction.

#### Scenario: Supported unblocked profile
- **WHEN** a caller previews or spawns a task with profile `unblocked` for a provider that advertises it
- **THEN** the selected provider adapter adds only that provider's known permission-bypass arguments and reports them in profile diagnostics.

#### Scenario: Unsupported unblocked profile
- **WHEN** a caller requests profile `unblocked` for a provider that does not advertise it
- **THEN** validation rejects the request before spawning a process and reports the provider/profile incompatibility.

#### Scenario: Workspace validation remains authoritative
- **WHEN** a caller requests profile `unblocked` with a `cwd` outside configured workspace roots
- **THEN** the bridge rejects the request before adding provider permission-bypass arguments or spawning a provider process.

### Requirement: Launch profile behavior is observable
The system SHALL expose launch profile metadata in provider capabilities, previews, task status or result metadata, and diagnostics.

#### Scenario: Listing profile capabilities
- **WHEN** a caller invokes `providers_list`
- **THEN** each provider includes supported launch profiles and reduced-configuration capability metadata.

#### Scenario: Inspecting completed profile task
- **WHEN** a caller reads `agent_result` for a task launched with a profile
- **THEN** the response includes the selected profile and profile diagnostics describing prompt strategy and configuration reductions.

### Requirement: Reduced-profile support is discovered by spike before implementation
The system SHALL include an implementation spike that empirically determines each provider's reduced prompt and configuration capabilities before finalizing provider-specific `bare` behavior.

#### Scenario: Spike output recorded
- **WHEN** the spike is completed
- **THEN** the change includes a recorded provider matrix covering provider version, validation method, compact prompts, custom system prompts, hook disabling, skill disabling, config isolation, memory/session disabling, auth preservation, and evidence for each provider.

#### Scenario: Provider behavior follows spike findings
- **WHEN** provider-specific `bare` launch behavior is implemented
- **THEN** adapter behavior and tests align with the recorded spike findings.
