---
status: accepted
date: 2025-04-??
---

# ADR-0004: Add Static Codebase Intelligence Gates

## Context

Without automated enforcement, entropy accumulates: dead dependencies linger, code duplication creeps upward, and cyclical module dependencies emerge unnoticed. Prior to this decision, there was no systematic barrier preventing these degradations from reaching `main`.

## Decision

We decided to add four hard CI gates mirrored by a local `scripts/quality.sh` script:

1. `cargo fmt --all --check` — format uniformity
2. `cargo clippy --all-targets -- -D warnings` — zero-warning correctness
3. `cargo machete` — no unused dependencies
4. `npx jscpd` — code duplication < 5%

Additionally, two informational-only reports run post-gate:

- `clippy` lints for cognitive complexity, too-many-lines, too-many-arguments
- `cargo modules dependencies` — acyclic check and module boundary visualization

These are intentionally non-blocking to avoid brittle false positives, but visible in every PR.

### Considered Alternatives

#### Rely on contributor discipline and periodic audits

- Good, because flexible and low-friction.
- Bad, because entropy wins; proven by prior accumulation of unused deps.

#### Fail on complexity/info reports too

- Good, because stricter hygiene.
- Bad, because `cargo-modules` incorrectly flags benign self-references (type aliases referencing their own module), leading to noisy red builds.

## Consequences

### Positive

- Cleanliness invariant is mechanically enforced.
- Local reproduction via `scripts/quality.sh` eliminates "works on my machine" surprises.

### Negative

- Contributors unfamiliar with Rust tooling may hit `machete` or `jscpd` failures unexpectedly.

### Neutral

- Node is required only for `jscpd`; fetched on-demand via `npx`.

## Evidence

- **Commit(s):** `9bcd957`
- **Key files changed:** `scripts/quality.sh`, `.github/workflows/quality.yml`
- **Blast radius:** 2 files, ~75 lines.
- **Timeline:** Single CI-focused PR.
