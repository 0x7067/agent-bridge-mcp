## ADDED Requirements

### Requirement: Runtime provider capabilities remain presentation-complete
The system SHALL keep runtime provider capability metadata complete enough for clients to render Agent Bridge presentation controls without relying on source files or README examples.

#### Scenario: Listing provider UI capabilities
- **WHEN** a caller invokes `providers_list`
- **THEN** each provider reports supported task modes, supported launch profiles, model or reasoning options when available, worktree isolation support, reply support, resume support, and reduced-configuration metadata when available.

#### Scenario: Runtime schema matches source capabilities
- **WHEN** a caller reads runtime provider capabilities from the installed MCP server
- **THEN** the response includes the same public capability categories that the source tool schema and documentation expose for launch profiles and provider action support.

#### Scenario: Unsupported provider action is explicit
- **WHEN** a provider does not support a native-client action such as reply or resume
- **THEN** provider capability metadata reports that action as unsupported rather than omitting it.

#### Scenario: Production binary capability drift
- **WHEN** the production MCP binary is exercised by the deterministic compatibility fixture
- **THEN** `providers_list` exposes the presentation-relevant capability categories expected by the source-level contract.

### Requirement: Provider metadata supports presentation-safe defaults
The system SHALL make provider defaults and unsupported combinations visible enough for clients to choose safe launch controls without trial-and-error tool calls.

#### Scenario: Client renders launch controls
- **WHEN** a client renders controls for spawning an Agent Bridge task
- **THEN** provider capability metadata indicates which modes, launch profiles, and provider-specific options are valid for the selected provider.

#### Scenario: Client avoids invalid command mode
- **WHEN** a provider does not support a mode such as `command`
- **THEN** provider capability metadata exposes that unsupported mode so the client can disable the control before spawning.

### Requirement: Runtime provider readiness is discoverable for launchable agents
The system SHALL expose a runtime provider readiness snapshot so clients can discover which Agent Bridge providers are currently launchable as agents after MCP startup without requiring protocol initialization to block on provider smoke probes.

#### Scenario: Startup exposes non-blocking readiness state
- **WHEN** an MCP client initializes Agent Bridge and then requests provider discovery metadata
- **THEN** the response distinguishes static provider capability metadata from runtime readiness states such as `stale`, `ready`, or `failed`.

#### Scenario: Version-only discovery does not imply launch readiness
- **WHEN** a provider binary is present but no task-path smoke probe has succeeded for the current runtime environment
- **THEN** the readiness snapshot does not mark that provider as startup verified or launchable by default.

#### Scenario: Smoke-verified provider is launchable
- **WHEN** a provider completes a task-path smoke probe successfully
- **THEN** the readiness snapshot marks the provider as startup verified and exposes it as launchable for compatible modes and launch profiles.

#### Scenario: Explicit refresh rediscover providers
- **WHEN** a caller requests provider rediscovery through the readiness check surface
- **THEN** the bridge refreshes the runtime readiness snapshot with checked timestamps, probe phase, timing fields, and diagnostics for each selected provider.

#### Scenario: Readiness failures remain actionable
- **WHEN** provider startup verification fails
- **THEN** the readiness snapshot includes the failure category and bounded diagnostics without removing the provider's static capabilities from the discovery response.
