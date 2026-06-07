## 1. Diagnostic Model

- [x] 1.1 Add binary diagnostic data structures for running, installed, and release binaries.
- [x] 1.2 Implement file metadata collection with exists/readable flags, size, modified time, and bounded FNV-1a fingerprint.
- [x] 1.3 Implement installed/release path selection with `AGENT_BRIDGE_INSTALLED_BIN` and `AGENT_BRIDGE_RELEASE_BIN` overrides.
- [x] 1.4 Implement freshness status and recommendations for missing, unreadable, matching, and differing binaries.
- [x] 1.5 Implement `matchesRunning`, `matchesRelease`, `fingerprintStatus`, and the status matrix from the design.

## 2. Doctor Integration

- [x] 2.1 Add `binary` to doctor responses.
- [x] 2.2 Add binary freshness recommendations to the existing recommendation list after setup/provider/client recommendations.
- [x] 2.3 Update the doctor output schema to include the additive `binary` section.
- [x] 2.4 Keep binary diagnostics out of top-level `summary.status` aggregation.

## 3. Documentation And Guidance

- [x] 3.1 Update README doctor documentation to describe binary freshness diagnostics.
- [x] 3.2 Update MCP guidance resources so callers know that binary freshness is diagnostic-only.

## 4. Tests

- [x] 4.1 Add unit tests for matching, differing, missing, and unreadable binary diagnostics.
- [x] 4.2 Add stdio doctor integration tests proving `doctor.binary` is present and existing sections remain present.
- [x] 4.3 Add tests proving binary diagnostics do not alter `summary.status`.
- [x] 4.4 Add tests proving binary diagnostics do not mutate binary files.
- [x] 4.5 Add tests for release path resolution from `doctor.cwd`, env overrides, skipped fingerprints, and binary recommendation ordering.

## 5. Verification

- [x] 5.1 Run `cargo test`.
- [x] 5.2 Run `cargo fmt --check`.
- [x] 5.3 Run `cargo clippy --all-targets -- -D warnings`.
- [x] 5.4 Run `openspec validate add-binary-freshness-doctor --strict`.
