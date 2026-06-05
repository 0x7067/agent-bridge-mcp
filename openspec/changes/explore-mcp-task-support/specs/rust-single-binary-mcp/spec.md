## ADDED Requirements

### Requirement: Rust binary negotiates task protocol support explicitly
The Rust MCP binary SHALL advertise protocol-level task support only when the selected MCP task compatibility surface is implemented and tested.

#### Scenario: No premature task capability
- **WHEN** a caller sends `initialize` before task support is implemented
- **THEN** the response does not advertise MCP task capabilities.

#### Scenario: Task capability fixture
- **WHEN** a supported task protocol surface is implemented
- **THEN** the stdio fixture verifies the exact task capability shape returned by `initialize`.

### Requirement: Rust binary handles task protocol methods safely
The Rust MCP binary SHALL route supported protocol task methods through the existing task manager without corrupting stdout or task state.

#### Scenario: Unsupported task method
- **WHEN** a caller sends an MCP task method that Agent Bridge does not support under the negotiated design
- **THEN** the server returns a JSON-RPC method-not-found or invalid-params error according to the compatibility design.

#### Scenario: Supported task method
- **WHEN** a caller sends a supported task method for a known task
- **THEN** the server returns protocol-compatible task state derived from the existing task registry.

#### Scenario: Unknown task id
- **WHEN** a caller sends a supported task method for an unknown task id
- **THEN** the server returns the protocol error selected by the compatibility design without panicking.
