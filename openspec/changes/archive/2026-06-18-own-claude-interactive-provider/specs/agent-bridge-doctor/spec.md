## MODIFIED Requirements

### Requirement: Doctor diagnoses Claude host-runner setup
The system SHALL report owned Claude host-runner configuration state separately from generic provider binary availability.

#### Scenario: Host runner not configured
- **WHEN** `AGENT_BRIDGE_CLAUDE_HOST_SOCKET` is not set
- **THEN** doctor reports host-runner status as not configured and explains that production Claude provider execution requires the owned host runner.
- **AND** Claude launch strategy metadata does not imply that direct Claude execution is a supported production path.

#### Scenario: Host runner configured and reachable
- **WHEN** `AGENT_BRIDGE_CLAUDE_HOST_SOCKET` is set and a bounded ping succeeds
- **THEN** doctor reports host-runner status as ok with protocol and workspace policy metadata.

#### Scenario: Host runner unavailable
- **WHEN** `AGENT_BRIDGE_CLAUDE_HOST_SOCKET` is set but the socket cannot be reached
- **THEN** doctor reports host-runner status as error within a bounded timeout and recommends starting or restarting the owned Claude host runner.

#### Scenario: Host runner protocol mismatch
- **WHEN** host-runner ping reports a protocol mismatch
- **THEN** doctor reports the mismatch and recommends upgrading or restarting the MCP binary and host runner together.

#### Scenario: Workspace policy mismatch
- **WHEN** host-runner ping reports a workspace policy mismatch
- **THEN** doctor reports the mismatch and recommends restarting the runner with matching `AGENT_BRIDGE_WORKSPACES`.
