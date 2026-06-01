## 1. Protocol Tests

- [x] 1.1 Add request-handler tests for initialization guidance capabilities, prompt listing/getting, resource listing/reading, and unknown guidance errors.
- [x] 1.2 Add stdio binary smoke coverage for the guidance capabilities and static guidance responses.
- [x] 1.3 Run the targeted tests and confirm they fail before implementation.

## 2. Guidance Implementation

- [x] 2.1 Add static prompt and resource definitions with hardcoded resource URI allowlisting.
- [x] 2.2 Advertise `prompts` and `resources` capabilities during initialization.
- [x] 2.3 Handle `prompts/list`, `prompts/get`, `resources/list`, and `resources/read` requests with MCP-compatible response shapes and JSON-RPC errors.

## 3. Documentation

- [x] 3.1 Document MCP self-description through prompts/resources and clarify client-dependent discovery behavior.

## 4. Verification

- [x] 4.1 Run `cargo fmt --check`.
- [x] 4.2 Run `cargo test`.
- [x] 4.3 Run `cargo clippy --all-targets -- -D warnings`.
- [x] 4.4 Run `openspec validate expose-mcp-usage-guidance`.
