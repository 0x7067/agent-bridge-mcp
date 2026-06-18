## MODIFIED Requirements

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
