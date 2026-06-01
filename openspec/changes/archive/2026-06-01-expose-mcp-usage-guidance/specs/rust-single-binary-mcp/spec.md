## ADDED Requirements

### Requirement: Rust binary exposes MCP guidance capabilities
The Rust MCP binary SHALL advertise and serve the MCP prompts and resources capabilities for static Agent Bridge usage guidance.

#### Scenario: Initialize advertises guidance capabilities
- **WHEN** a caller sends `initialize` over stdio to the Rust binary
- **THEN** the response advertises `tools`, `prompts`, and `resources` capabilities while preserving the supported protocol version.

#### Scenario: Guidance methods over stdio
- **WHEN** a caller sends `prompts/list`, `prompts/get`, `resources/list`, and `resources/read` over stdio to the Rust binary
- **THEN** the responses use MCP-compatible prompt and resource response shapes.

#### Scenario: Existing tools remain available
- **WHEN** a caller sends `tools/list` after guidance capability support is added
- **THEN** the existing provider and task lifecycle tools remain listed with their current public names.
