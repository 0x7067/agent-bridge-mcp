## Context

Five provider-specific concerns each branch on `ProviderKind` in different functions across `provider.rs` and `task.rs`. The branching is correct today but does not scale: the pending `add-antigravity-provider` change must touch every site, and provider behavior cannot be read or tested in isolation. Idiomatic Rust addresses this with a trait plus a registry of trait objects.

## Goals / Non-Goals

- Goals: one place per provider; core lifecycle free of `ProviderKind` branching; behavior-preserving migration; smaller core files.
- Non-Goals: dynamic library loading of providers, changing provider CLI contracts, or altering the tool surface.

## Decisions

- **Trait object registry over generics.** Provider count is small but resolved at runtime from request input, so `Box<dyn ProviderAdapter>` in a registry keyed by `ProviderKind` is the right fit; generics would force monomorphization the call sites cannot use.
- **Five-method surface.** `capabilities`, `validate_options`, `build_command`, `profile_flags`, and `analyze_completion` cover every current branch point. `analyze_completion` takes exit code plus stdout/stderr and returns a completion hint, subsuming Codex denial detection and Claude output checks.
- **Behavior equivalence is the acceptance bar.** Migration is a refactor: per-provider tests must assert the new adapter produces the same command and completion verdict as the current code before the old branches are deleted.
- **Module extraction follows the trait.** Once providers are behind the trait, the doctor/providers/supervision helpers no longer need to live beside the lifecycle code; extracting them shrinks `server.rs` and `task.rs` and isolates adapter usage.

## Risks / Trade-offs

- A large refactor risks behavioral drift; mitigate with equivalence tests landed before deletion of old paths.
- Trait dispatch adds a small indirection cost, negligible relative to subprocess spawn latency.
- No backwards-compatibility shim is kept (per project direction), so the migration must land atomically per provider with tests green.

## Migration Plan

Per-provider, Codex first as the reference. For each provider: implement the adapter, add equivalence tests, switch dispatch, delete the old branch. Module extraction lands last, after all `ProviderKind` branching is gone from core code.
