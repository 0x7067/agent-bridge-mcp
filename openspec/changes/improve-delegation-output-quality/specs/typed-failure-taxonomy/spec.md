# typed-failure-taxonomy Specification

## Purpose
Replace all stringly-typed failure categories with a strongly typed enum, guaranteeing exhaustiveness checking and eliminating typo risk in diagnostics, retry logic, and host-runner wire format.

## ADDED Requirements

### Requirement: FailureCategory enum covers all known categories
The system SHALL define a `FailureCategory` enum encompassing every failure category presently expressed as a task or provider failure string, including `ProviderTimeout`, `ProviderExitError`, `ProviderStartError`, `ProviderOutputError`, `ProviderSandboxDenied`, `HostRunnerUnavailable`, `WorktreeCleanupFailed`, `WorktreeReclaimFailed`, `TranscriptUnavailable`, `AgentDirCleanupFailed`, and `ClientDisconnected`.

#### Scenario: Enum completeness guards against omissions
- **WHEN** a developer adds a new failure pathway
- **THEN** the compiler mandates a corresponding `FailureCategory` variant before the code compiles.

### Requirement: Serialization maps to stable snake_case strings
The system SHALL serialize `FailureCategory` to snake_case strings matching the historical wire format (e.g., `provider_timeout`). Deserialization SHALL round-trip the same mapping.

#### Scenario: Round-trip stability
- **WHEN** a `FailureCategory` is serialized to JSON and deserialized back
- **THEN** the resulting variant equals the original.

#### Scenario: Historical string compatibility
- **WHEN** the host runner transmits `"provider_timeout"` in a v2 response
- **THEN** the server deserializes it to `FailureCategory::ProviderTimeout`.

### Requirement: All diagnostic structs adopt the enum
The system SHALL replace every occurrence of `failure_category: Option<&'static str>` and `Option<String>` with `Option<FailureCategory>` in `ProbeResult`, `HostRunResult`, `TaskCompletion`, and all diagnostic builder functions.

#### Scenario: Provider diagnostic uses enum
- **WHEN** `provider_diagnostic` constructs a JSON diagnostic blob
- **THEN** the `failureCategory` field is populated by serializing `FailureCategory`, not by copying a raw string.

#### Scenario: Task record stores typed category
- **WHEN** a task finalizes
- **THEN** the `diagnostic["failureCategory"]` value originates from the enum, not a free-text constant.
