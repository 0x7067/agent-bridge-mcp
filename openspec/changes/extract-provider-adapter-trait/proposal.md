## Why

Provider-specific knowledge is hardcoded and scattered. Option validation (`provider.rs:233-262`), command construction (a 115-line match at `provider.rs:401-516`), launch-profile flags (`provider.rs:672-695`), and completion analysis such as Codex denial detection and Claude output parsing (`task.rs:1177-1208`, `task.rs:1316-1337`) each branch on `ProviderKind`. Adding a provider means editing four-plus functions across two files, and provider behavior cannot be reasoned about in one place. A trait-based adapter consolidates each provider's CLI contract and failure heuristics behind one interface, making providers pluggable and the core lifecycle provider-agnostic. This also enables splitting the monolithic `server.rs` (3056 lines) and `task.rs` (3085 lines) along clean seams.

## What Changes

- Define a `ProviderAdapter` trait capturing per-provider behavior: capability declaration, option validation, command construction, launch-profile flags, and completion analysis.
- Implement the trait for each existing provider (Cursor, Kimi, Codex, Claude, Antigravity) as a self-contained unit, moving all `ProviderKind` branching out of core lifecycle code.
- Resolve adapters through a registry keyed by provider so core code calls trait methods generically.
- Remove the scattered `ProviderKind` match arms from `provider.rs` and `task.rs` in favor of adapter dispatch (no backwards-compatibility shim required, per project direction).
- Extract `doctor`/readiness and provider-smoke logic from `server.rs` into dedicated modules, and the supervision/cleanup helpers from `task.rs`, to reduce file size and isolate adapter usage.

## Capabilities

### New Capabilities

- `provider-adapter-abstraction`: Covers the `ProviderAdapter` trait, the provider registry, and the requirement that core lifecycle code dispatch provider behavior through the trait rather than inline `ProviderKind` branching.

### Modified Capabilities

- `rust-single-binary-mcp`: The Rust MCP binary must build commands, validate options, and analyze completion through the provider abstraction while preserving existing tool behavior and outputs.

## Impact

- Affected code: `crates/agent-bridge-mcp/src/provider.rs` (trait + per-provider impls), `crates/agent-bridge-mcp/src/task.rs` (completion analysis dispatch, module extraction), `crates/agent-bridge-mcp/src/server.rs` (module extraction for doctor/providers).
- Affected APIs: none externally; tool inputs, outputs, and provider names are unchanged.
- Affected docs/specs: README provider section and any provider-extension guidance.
- Dependencies: no new third-party dependency is expected.
