## ADDED Requirements

### Requirement: Rust binary negotiates task protocol support explicitly
The Rust MCP binary SHALL advertise protocol-level task support only when the selected MCP task compatibility surface is implemented and tested.

#### Scenario: No premature task capability
- **WHEN** a caller sends `initialize` before task support is implemented
- **THEN** the response does not advertise MCP task capabilities.

### Requirement: Rust binary handles task protocol methods safely
The Rust MCP binary SHALL reject unsupported protocol task methods without corrupting stdout or task state.

#### Scenario: Unsupported task method
- **WHEN** a caller sends an MCP task method that Agent Bridge does not support under the negotiated design
- **THEN** the server returns a JSON-RPC method-not-found or invalid-params error according to the compatibility design.
