## ADDED Requirements

### Requirement: Provider behavior is defined behind a trait
The system SHALL define a `ProviderAdapter` trait that encapsulates per-provider capability declaration, option validation, command construction, launch-profile flags, and completion analysis.

#### Scenario: Each provider implements the trait
- **WHEN** a provider is supported by Agent Bridge
- **THEN** that provider has a `ProviderAdapter` implementation
- **AND** its CLI flags and completion heuristics live in that implementation.

### Requirement: Adapters are resolved through a registry
The system SHALL resolve a provider's adapter through a registry keyed by provider.

#### Scenario: Unknown provider
- **WHEN** core code requests an adapter for an unsupported provider
- **THEN** resolution fails with a clear error rather than panicking.

### Requirement: Core lifecycle dispatches through the trait
Core lifecycle code SHALL build commands, validate options, and analyze completion through the provider adapter rather than inline provider branching.

#### Scenario: Command construction is provider-agnostic
- **WHEN** a task is launched for any supported provider
- **THEN** the command is produced by that provider's adapter
- **AND** core launch code contains no provider-specific branching.

#### Scenario: Completion analysis is provider-agnostic
- **WHEN** a child process exits
- **THEN** the success or denial verdict is produced by the provider's adapter
- **AND** the resulting status matches the behavior of the pre-refactor implementation for the same inputs.

### Requirement: Refactor preserves provider behavior
The migration SHALL preserve existing observable behavior for every provider.

#### Scenario: Equivalence before deletion
- **WHEN** a provider is migrated to its adapter
- **THEN** tests assert the adapter produces the same command and completion verdict as the prior implementation
- **AND** the prior provider-specific branch is removed only after those tests pass.
