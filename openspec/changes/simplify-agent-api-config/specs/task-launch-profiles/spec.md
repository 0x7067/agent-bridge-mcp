## MODIFIED Requirements

### Requirement: Tasks support explicit launch profiles
The system SHALL allow callers to select an agent launch profile that controls prompt wrapping and provider configuration strategy.

#### Scenario: Previewing a launch profile
- **WHEN** a caller invokes `agent_preview` with a supported launch profile
- **THEN** the response includes the selected profile, profile-specific command metadata, prompt strategy metadata, and any provider-specific reduction diagnostics.

#### Scenario: Spawning with a launch profile
- **WHEN** a caller invokes `agent_spawn` with a supported launch profile
- **THEN** the provider adapter launches the agent using that profile and the agent lifecycle records the selected profile.

#### Scenario: Unsupported launch profile
- **WHEN** a caller requests a launch profile unsupported by the selected provider
- **THEN** validation rejects the request before spawning a process and reports the provider/profile incompatibility.

### Requirement: Launch profile behavior is observable
The system SHALL expose launch profile metadata in provider capabilities, previews, agent status or result metadata, and diagnostics.

#### Scenario: Listing profile capabilities
- **WHEN** a caller invokes `providers_list`
- **THEN** each provider includes supported launch profiles and reduced-configuration capability metadata.

#### Scenario: Inspecting completed profile task
- **WHEN** a caller reads `agent_result` for an agent launched with a profile
- **THEN** the response includes the selected profile and profile diagnostics describing prompt strategy and configuration reductions.
