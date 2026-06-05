## Why

Agent Bridge now exposes a rich provider task lifecycle, but callers still need to remember too much orchestration: when to run smoke checks, how to monitor a task, when to inspect results, and when cleanup is safe. Current MCP and Codex surfaces can carry more of that guidance directly through initialization instructions, structured tool results, and ranked next actions.

## What Changes

- Add concise MCP initialization instructions for Agent Bridge's cross-tool workflow, safety boundary, and cleanup rule.
- Add structured tool results and output schemas for core lifecycle tools while preserving the existing text content for compatibility.
- Add ranked `nextActions` metadata to task presentation/result surfaces so callers can render or follow the most appropriate lifecycle step with ready-to-call tool arguments.
- Refine `doctor` readiness semantics so a green setup report does not obscure provider readiness states such as available-but-not-launchable.
- Update guidance resources, prompts, README examples, and tests to prefer the self-guiding structured flow.
- Preserve existing tool names and lifecycle semantics; this change is additive.

## Capabilities

### New Capabilities

- `agent-bridge-self-guidance`: Covers MCP initialization instructions, structured tool outputs, ranked next-action metadata, and clearer readiness guidance that make Agent Bridge easier for clients and models to use correctly.

### Modified Capabilities

- `agent-bridge-doctor`: Doctor readiness output must distinguish setup health from launch readiness and recommend smoke checks when providers are available but not startup-verified.
- `mcp-usage-guidance`: Runtime guidance must describe the self-guiding structured workflow and remain aligned with initialization instructions.
- `rust-single-binary-mcp`: The Rust MCP public protocol surface must include initialization instructions and structured tool-result compatibility coverage.

## Impact

- Affected code: MCP initialization and result shaping in `crates/agent-bridge-mcp/src/server.rs`, tool definitions in `crates/agent-bridge-mcp/src/tools.rs`, task presentation/review-packet derivation in `crates/agent-bridge-mcp/src/task.rs`, and guidance text in `crates/agent-bridge-mcp/src/guidance.rs`.
- Affected APIs: additive `initialize.instructions`, additive `structuredContent` fields, additive `outputSchema` tool metadata, additive task `presentation.nextActions`/result `nextActions` metadata, and refined doctor readiness fields/recommendations.
- Affected docs/specs: README workflow examples, MCP usage guidance, self-guidance contract, doctor contract, and Rust binary protocol fixtures.
- Dependencies: no new third-party dependency is expected.
