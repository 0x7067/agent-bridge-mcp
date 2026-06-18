## ADDED Requirements

### Requirement: MCP task support is compatibility-gated
The system SHALL require an explicit compatibility decision before advertising or implementing protocol-level MCP task support.

#### Scenario: Compatibility memo precedes task capability advertisement
- **WHEN** MCP task support implementation begins
- **THEN** the change includes a documented compatibility decision naming the targeted MCP version, capability shape, supported methods, and unsupported methods.

#### Scenario: No unsupported task claims
- **WHEN** a client sends `initialize` before MCP task compatibility is implemented
- **THEN** Agent Bridge does not advertise task capabilities it does not implement.

### Requirement: MCP task protocol methods remain unavailable until supported
The system SHALL reject protocol-level MCP task methods until a compatible task surface is implemented and advertised.

#### Scenario: Task methods are unsupported
- **WHEN** a client calls `tasks/get`, `tasks/update`, `tasks/cancel`, `tasks/list`, or `tasks/result` before MCP task compatibility is implemented
- **THEN** Agent Bridge returns a JSON-RPC method-not-found error.

#### Scenario: No task tool aliases
- **WHEN** a client lists tools before MCP task compatibility is implemented
- **THEN** Agent Bridge does not expose task protocol methods as tools.

### Requirement: Existing Agent Bridge lifecycle remains primary and stable
The system SHALL preserve the existing Agent Bridge `agent_*` lifecycle tools regardless of MCP task support.

#### Scenario: Legacy lifecycle remains available
- **WHEN** a client does not advertise MCP task support
- **THEN** the existing `agent_*` tools continue to provide spawn, list, observe, result, stop, and cleanup behavior.

#### Scenario: MCP task support is additive
- **WHEN** a client advertises supported MCP task capabilities
- **THEN** Agent Bridge continues to recommend the native `agent_*` lifecycle until protocol task support is implemented and advertised.
