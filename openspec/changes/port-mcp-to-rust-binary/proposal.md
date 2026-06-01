## Why

The former MCP server worked, but its most important contracts were enforced through runtime validation, stringly JSON state, and tests that imported implementation modules directly. Rewriting the MCP server in Rust makes protocol inputs, task lifecycle state, provider behavior, and persisted records type-safe while also producing a single built MCP executable.

## What Changes

- Add a Rust implementation of the MCP server that preserves the current public task lifecycle API.
- Introduce typed Rust models for providers, task modes, task states, error types, tool inputs, tool outputs, command descriptors, and persisted registry records.
- Build stdio golden compatibility fixtures that captured legacy behavior during migration and now run against the Rust binary.
- Preserve provider adapter behavior, including command arguments, environment allowlists, Claude shell initialization, provider checks, and startup smoke probes.
- Preserve current state directory defaults, task directory layout, registry compatibility, stale task recovery, log caps, git snapshots, worktree cleanup semantics, and task lifecycle response shapes.
- Ship the long-term MCP entrypoint as a single built binary named `agent-bridge-mcp`.
- Allow a short side-by-side transition command, such as `agent-bridge-mcp-rs`, only while compatibility fixtures are being proven.
- Use direct binary releases as the packaging path.
- No intentional public API break. Any unavoidable break must be called out with a migration path before implementation.

## Capabilities

### New Capabilities
- `rust-single-binary-mcp`: Covers the Rust MCP server, type-safe domain model, migration strategy, packaging expectations, and final single-binary runtime contract.

### Modified Capabilities
- None. The existing `provider-adapter-contract` remains behaviorally unchanged and must be preserved by the Rust implementation.

## Impact

- Affected code: Rust crate and binary entrypoint, `Cargo.toml`, `crates/agent-bridge-mcp/`, packaging files, and removal of the former runtime after parity.
- Affected tests: Rust stdio, lifecycle, and unit tests.
- Affected docs: README install/config examples must describe the final binary and any transition command.
- Runtime dependencies: the MCP server becomes a single binary, but provider CLIs and `git` remain external runtime dependencies.
- Source-of-truth references: MCP stdio transport and schema requirements come from the official MCP specification; Rust SDK use must be evaluated because the official docs currently list Rust as Tier 2.
