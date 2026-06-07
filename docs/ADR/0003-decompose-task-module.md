---
status: accepted
date: 2025-04-??
---

# ADR-0003: Decompose Monolithic task.rs into Five Focused Submodules

## Context

`task.rs` had grown to approximately 110 KB, mixing registry persistence, spawn logic, child supervision, completion classification, and review payload formatting in one file. Cognitive load for contributors was high; the file exceeded reasonable screen/editor navigation bounds. Clippy and human reviewers alike struggled with the sheer density.

## Decision

We decided to split `task.rs` into five submodule files under `src/task/`:

- `registry.rs` — JSON registry load/save, temp cleanup, home expansion, normalization
- `spawn.rs` — Argument validation, worktree creation, process launch, host-runner bridging
- `supervision.rs` — PID tracking, process groups, signal propagation, IO draining
- `complete.rs` — Exit-status classification, host-response ingestion, transcript scanning
- `review.rs` — Progress computation, payload shaping, `next` action list generation, listing/querying

Public re-exports remained on `task.rs` so consumers (`server.rs`, `runtime.rs`) did not change.

### Considered Alternatives

#### Keep the monolith but add aggressive doc comments

- Good, because avoids import plumbing changes.
- Bad, because does not solve compilation-unit bloat or merge-conflict frequency.

#### Split into separate crates

- Good, because stronger visibility boundaries.
- Bad, because unnecessary for a single-package workspace; increases build graph complexity.

## Consequences

### Positive

- Average file size drops to ~200–800 lines per concern.
- Parallel compilation gains (within crate limits).
- Easier to locate logic when grepping or navigating.

### Negative

- `pub(super)` / `pub(crate)` visibility discipline must be maintained; accidental leakage possible.
- Circular-import risk between `complete.rs` and `supervision.rs` (mitigated by strict DAG).

### Neutral

- `task.rs` now acts primarily as a facade and `TaskManagerHandle` definition site.

## Evidence

- **Commit(s):** `21a28df`
- **Key files changed:** `src/task.rs`, `src/task/complete.rs`, `src/task/registry.rs`, `src/task/review.rs`, `src/task/spawn.rs`, `src/task/supervision.rs`
- **Blast radius:** 6 files, ~2050 lines inserted/~2040 lines deleted (pure shuffle).
- **Timeline:** Single focused refactor PR.
