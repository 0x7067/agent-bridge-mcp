## 1. Diagnostic Model

- [ ] 1.1 Add binary diagnostic data structures for running, installed, and release binaries.
- [ ] 1.2 Implement file metadata collection with exists/readable flags, size, modified time, and bounded FNV-1a fingerprint.
- [ ] 1.3 Implement installed/release path selection with `AGENT_BRIDGE_INSTALLED_BIN` and `AGENT_BRIDGE_RELEASE_BIN` overrides.
- [ ] 1.4 Implement freshness status and recommendations for missing, unreadable, matching, and differing binaries.
- [ ] 1.5 Implement `matchesRunning`, `matchesRelease`, `fingerprintStatus`, and the status matrix from the design.

## 2. Doctor Integration

- [ ] 2.1 Add `binary` to doctor responses.
- [ ] 2.2 Add binary freshness recommendations to the existing recommendation list after setup/provider/client recommendations.
- [ ] 2.3 Update the doctor output schema to include the additive `binary` section.
- [ ] 2.4 Keep binary diagnostics out of top-level `summary.status` aggregation.

## 3. Documentation And Guidance

- [ ] 3.1 Update README doctor documentation to describe binary freshness diagnostics.
- [ ] 3.2 Update MCP guidance resources so callers know that binary freshness is diagnostic-only.

## 4. Tests

- [ ] 4.1 Add unit tests for matching, differing, missing, and unreadable binary diagnostics.
- [ ] 4.2 Add stdio doctor integration tests proving `doctor.binary` is present and existing sections remain present.
- [ ] 4.3 Add tests proving binary diagnostics do not alter `summary.status`.
- [ ] 4.4 Add tests proving binary diagnostics do not mutate binary files.
- [ ] 4.5 Add tests for release path resolution from `doctor.cwd`, env overrides, skipped fingerprints, and binary recommendation ordering.

## 5. Verification

- [ ] 5.1 Run `cargo test`.
- [ ] 5.2 Run `cargo fmt --check`.
- [ ] 5.3 Run `cargo clippy --all-targets -- -D warnings`.
- [ ] 5.4 Run `openspec validate add-binary-freshness-doctor --strict`.
