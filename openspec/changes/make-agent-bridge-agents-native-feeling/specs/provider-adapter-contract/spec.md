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
