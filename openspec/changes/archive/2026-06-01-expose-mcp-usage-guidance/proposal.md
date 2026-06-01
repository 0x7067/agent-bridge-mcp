## Why

Agent Bridge's safest caller workflow is documented in the README, but MCP clients only discover the tool schemas at runtime. The server should expose concise usage guidance through MCP-native channels so agents and clients can discover when and how to delegate without relying on out-of-band docs.

## What Changes

- Advertise self-description through MCP initialization instructions.
- Add MCP prompt templates for common delegation workflows.
- Add MCP resources containing longer caller workflow, safety, and provider capability guidance.
- Keep the existing task lifecycle tools and provider behavior unchanged.
- Document the new self-description surface and its client-dependent discovery semantics.

## Capabilities

### New Capabilities
- `mcp-usage-guidance`: Server-discoverable instructions, prompts, and resources that explain when and how to use Agent Bridge safely.

### Modified Capabilities
- `rust-single-binary-mcp`: The Rust MCP server public protocol surface now includes prompts/resources capabilities and their list/get/read methods.

## Impact

- Affected code: `crates/agent-bridge-mcp/src/server.rs`, protocol helpers or tests if new response builders are needed.
- Affected tests: stdio and request-handler protocol coverage for initialization, `prompts/list`, `prompts/get`, `resources/list`, and `resources/read`.
- Affected docs: `README.md`.
- No new runtime dependencies or provider CLI changes.
