## 1. Trait Definition

- [x] 1.1 Define a `ProviderAdapter` trait with methods for capabilities, option validation, command construction, launch-profile flags, and completion analysis.
- [x] 1.2 Define the shared input/output types the trait uses (provider task, provider command, completion hint) reusing existing structs where possible.
- [x] 1.3 Add a `ProviderRegistry` resolving an adapter by provider. (Implemented as `adapter_for(provider)`.)

## 2. Per-Provider Implementations

- [x] 2.1 Implement `ProviderAdapter` for Codex (reference implementation), moving its flags (`provider.rs:672-695`) and denial detection (`task.rs:1177-1208`) into the impl.
- [x] 2.2 Implement for Claude, moving output-parseable checks (`task.rs:1316-1337`) into the impl.
- [x] 2.3 Implement for Kimi, Cursor, and Antigravity.
- [x] 2.4 Add per-provider unit tests for command construction and completion analysis equivalence with current behavior.

## 3. Core Dispatch Migration

- [x] 3.1 Replace `build_command` match (`provider.rs:401-516`) with adapter dispatch.
- [x] 3.2 Replace `validate_options` provider branches (`provider.rs:233-262`) with adapter dispatch.
- [x] 3.3 Replace inline `ProviderKind` completion branching in `wait_for_child` with `analyze_completion` dispatch.
- [x] 3.4 Remove now-dead `ProviderKind` match arms.

## 4. Module Extraction

- [x] 4.1 Extract doctor/readiness helpers from `server.rs` into a module. (Consolidated with 4.2 into `server/diagnostics.rs`; doctor + providers helpers are mutually coupled, so one cohesive module is cleaner than two. `server.rs`: 3056 → 1175 lines.)
- [x] 4.2 Extract provider-smoke/check helpers from `server.rs` into a module. (See 4.1 — same `server/diagnostics.rs`.)
- [x] 4.3 Extract child-supervision/cleanup helpers from `task.rs` into a focused module. (Moved to `task/supervision.rs`; the PID registry is redesigned as a testable `ActivePids` value — the process-global is one instance, tests use isolated instances — which removes the cross-test signal hazard and restores real signal-delivery coverage.)

## 5. Verification

- [x] 5.1 Run `cargo test` (including provider equivalence tests).
- [x] 5.2 Run `cargo fmt --check`.
- [x] 5.3 Run `cargo clippy --all-targets -- -D warnings`.
- [x] 5.4 Run `openspec validate extract-provider-adapter-trait --strict`.
