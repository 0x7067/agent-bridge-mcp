## ADDED Requirements

### Requirement: Rust binary advertises a simplified primary tool workflow
The Rust MCP binary SHALL keep the current public tools callable while describing primary and diagnostic tools distinctly.

#### Scenario: Tool list preserves callable surface
- **WHEN** a caller sends `tools/list`
- **THEN** the response includes the existing provider, setup, and `agent_*` lifecycle tools.
- **AND** the response does not include legacy duplicate `task_*` lifecycle tools.

#### Scenario: Tool descriptions distinguish diagnostic tools
- **WHEN** a caller inspects `tools/list`
- **THEN** primary lifecycle tools such as `agent_spawn`, `agent_observe`, and `agent_result` have descriptions aligned with the compact workflow.
- **AND** focused or lower-level tools such as `providers_check`, `agent_preview`, `agent_status`, `agent_wait`, `agent_logs`, and `agent_transcript` are described as readiness, preview, status, finality, raw evidence, or transcript inspection surfaces.

#### Scenario: Public schema compatibility is preserved
- **WHEN** existing callers invoke any current public tool with valid arguments
- **THEN** the Rust binary preserves current strict input schemas, output compatibility, and tool-level error semantics.
