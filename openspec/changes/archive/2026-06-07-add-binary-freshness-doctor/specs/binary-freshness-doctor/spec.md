## ADDED Requirements

### Requirement: Doctor reports binary freshness diagnostics
The system SHALL report read-only diagnostics for Agent Bridge binary freshness.

#### Scenario: Default binary diagnostics
- **WHEN** a caller invokes `doctor`
- **THEN** the response includes a `binary` section.
- **AND** the section includes `status`, `running`, `installed`, `release`, and `recommendations` fields.

#### Scenario: Binary metadata shape
- **WHEN** binary diagnostics report a running, installed, or release binary target
- **THEN** that target includes `path`, `exists`, `readable`, `sizeBytes`, `modifiedAt`, `fingerprint`, `fingerprintStatus`, and optional `error` fields.

#### Scenario: Running executable reported
- **WHEN** the current process executable path can be resolved
- **THEN** `binary.running.path` reports that path with file metadata when readable.

#### Scenario: Running executable comparison
- **WHEN** the running executable and installed binary are both readable
- **THEN** `binary.installed.matchesRunning` reports whether their size and fingerprint match.

#### Scenario: Release executable comparison
- **WHEN** the running executable and release candidate are both readable
- **THEN** `binary.release.matchesRunning` reports whether their size and fingerprint match.

#### Scenario: Binary diagnostics are read-only
- **WHEN** a caller invokes `doctor`
- **THEN** binary diagnostics do not build, copy, install, delete, or modify binary files.

### Requirement: Binary diagnostics compare installed and release binaries
The system SHALL compare the installed binary and release candidate when both files are readable.

#### Scenario: Installed binary matches release candidate
- **WHEN** the installed binary and release candidate have the same size and fingerprint
- **THEN** `binary.installed.matchesRelease` is true.
- **AND** `binary.status` is `ok` unless another binary diagnostic issue is present.

#### Scenario: Installed binary differs from release candidate
- **WHEN** the installed binary and release candidate are both readable but their size or fingerprint differs
- **THEN** `binary.installed.matchesRelease` is false.
- **AND** `binary.status` is `warning`.
- **AND** doctor recommends rebuilding and installing the release binary.

#### Scenario: Binary recommendation shape
- **WHEN** doctor recommends binary build or install follow-up
- **THEN** `binary.recommendations` includes concise section-local recommendation strings.
- **AND** top-level `recommendations` includes structured entries with `kind: "shell"` and a `command` array when a concrete command is known.

#### Scenario: Release candidate missing
- **WHEN** the release candidate path does not exist
- **THEN** binary diagnostics report the release candidate as missing.
- **AND** doctor recommends running the documented release build before comparing freshness.

#### Scenario: Installed binary missing
- **WHEN** the installed binary path does not exist
- **THEN** binary diagnostics report the installed binary as missing.
- **AND** doctor recommends installing the release binary.

#### Scenario: Binary status rules
- **WHEN** binary diagnostics classify freshness
- **THEN** `binary.status` is `ok` for a readable installed binary matching the release candidate.
- **AND** `binary.status` is `warning` for missing binaries, differing binaries, running-vs-installed mismatch, release-vs-installed mismatch, or skipped fingerprints.
- **AND** `binary.status` is `error` when the current executable cannot be resolved or a configured override path is unreadable for reasons other than missing.
- **AND** `binary.status` is `unknown` when there is insufficient metadata to compare and no stronger warning or error applies.

#### Scenario: Fingerprint read cap
- **WHEN** a binary target is larger than the fingerprint read cap
- **THEN** binary diagnostics set `fingerprintStatus` to `skipped_too_large`.
- **AND** doctor does not read the file contents for fingerprinting.

### Requirement: Binary diagnostics avoid false verification claims
The system SHALL keep binary freshness separate from project and provider verification.

#### Scenario: Binary status does not affect summary status
- **WHEN** binary diagnostics report `ok`, `warning`, `error`, or `unknown`
- **THEN** the binary status does not change `summary.status`.

#### Scenario: Binary diagnostics do not verify tasks
- **WHEN** doctor reports binary freshness diagnostics
- **THEN** it does not claim delegated task output, provider model behavior, project tests, or build freshness beyond the compared files are verified.

### Requirement: Binary diagnostic paths are configurable by environment
The system SHALL support environment-controlled path overrides for deterministic tests and non-default installs.

#### Scenario: Installed path override
- **WHEN** `AGENT_BRIDGE_INSTALLED_BIN` is set
- **THEN** binary diagnostics use that path as the installed binary path.

#### Scenario: Release path override
- **WHEN** `AGENT_BRIDGE_RELEASE_BIN` is set
- **THEN** binary diagnostics use that path as the release candidate path.

#### Scenario: Release path from doctor cwd
- **WHEN** `AGENT_BRIDGE_RELEASE_BIN` is not set and `doctor.cwd` is provided
- **THEN** binary diagnostics use `<doctor cwd>/target/release/agent-bridge-mcp` as the release candidate path.

#### Scenario: Release path from process cwd
- **WHEN** `AGENT_BRIDGE_RELEASE_BIN` is not set and `doctor.cwd` is not provided
- **THEN** binary diagnostics use `<process cwd>/target/release/agent-bridge-mcp` as the release candidate path.
