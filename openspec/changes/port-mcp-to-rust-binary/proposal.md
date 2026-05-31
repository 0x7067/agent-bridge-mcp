## Why

The current Node.js MCP server works, but its most important contracts are still enforced through runtime validation, stringly JSON state, and tests that import implementation modules directly. Rewriting the MCP server in Rust can make protocol inputs, task lifecycle state, provider behavior, and persisted records type-safe while also producing a single built MCP executable.

## What Changes

- Add a Rust implementation of the MCP server that preserves the current public task lifecycle API.
- Introduce typed Rust models for providers, task modes, task states, error types, tool inputs, tool outputs, command descriptors, and persisted registry records.
- Build stdio golden compatibility fixtures that run against both the current Node server and the Rust binary before behavior is migrated.
- Preserve provider adapter behavior from the Node implementation, including command arguments, environment allowlists, Claude shell initialization, provider checks, and startup smoke probes.
- Preserve current state directory defaults, task directory layout, registry compatibility, stale task recovery, log caps, git snapshots, worktree cleanup semantics, and task lifecycle response shapes.
- Ship the long-term MCP entrypoint as a single built binary named `agent-bridge-mcp`.
- Allow a short side-by-side transition command, such as `agent-bridge-mcp-rs`, only while compatibility fixtures are being proven.
- Use direct binary releases as the first packaging path. If npm install UX is retained, npm must be a thin platform-specific prebuilt binary launcher, not the MCP runtime.
- No intentional public API break. Any unavoidable break must be called out with a migration path before implementation.

## Capabilities

### New Capabilities
- `rust-single-binary-mcp`: Covers the Rust MCP server, type-safe domain model, Node/Rust compatibility strategy, packaging expectations, and final single-binary runtime contract.

### Modified Capabilities
- None. The existing `provider-adapter-contract` remains behaviorally unchanged and must be preserved by the Rust implementation.

## Impact

- Affected code: new Rust crate and binary entrypoint, likely `Cargo.toml` and `src-rs/` or `crates/agent-bridge-mcp/`; existing Node files during transition; packaging files.
- Affected tests: add stdio golden fixtures and Rust tests; keep Node tests until parity is proven.
- Affected docs: README install/config examples must describe the final binary and any transition command.
- Runtime dependencies: the MCP server becomes a single binary, but provider CLIs and `git` remain external runtime dependencies.
- Source-of-truth references: MCP stdio transport and schema requirements come from the official MCP specification; Rust SDK use must be evaluated because the official docs currently list Rust as Tier 2.
