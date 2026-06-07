# universal-output-validation Specification

## Purpose
Require every `ProviderAdapter` to validate that a provider's output conforms to the expected wire format before declaring success, closing the gap where only Claude currently enforces parseability.

## ADDED Requirements

### Requirement: ProviderAdapter mandates acceptance reporting
The system SHALL extend `ProviderAdapter` with `fn acceptance_report(&self, stdout: &[u8], stderr: &[u8]) -> AcceptanceReport`.

#### Scenario: Default adapter defers to permissive
- **WHEN** a provider adapter does not override `acceptance_report`
- **THEN** the default implementation returns `AcceptanceReport { acceptable: true, reason: None, category: None }`.

#### Scenario: Claude adapter validates JSON parseability
- **WHEN** Claude produces stdout lacking a parseable JSON line with a non-empty `result` field
- **THEN** `acceptance_report` returns `acceptable: false` with `category: Some(FailureCategory::ProviderOutputError)`.

#### Scenario: Codex adapter validates nonzero exit with denial
- **WHEN** Codex exits 0 but stderr contains a sandbox denial phrase
- **THEN** `acceptance_report` considers the output unacceptable and surfaces the denial category.

### Requirement: Classification respects acceptance report on success exits
The system SHALL invoke `acceptance_report` during `classify_success_exit` before concluding `TaskStatus::Succeeded`.

#### Scenario: Exit zero with bad output fails
- **WHEN** a provider exits 0 but its adapter reports `acceptable: false`
- **THEN** the task transitions to `TaskStatus::Failed` with the reported category and reason.

#### Scenario: Exit zero with good output succeeds
- **WHEN** a provider exits 0 and its adapter reports `acceptable: true`
- **THEN** the task transitions to `TaskStatus::Succeeded` normally.

### Requirement: Acceptance report populates diagnostics
The system SHALL include the refusal reason and category in the task diagnostic when acceptance fails.

#### Scenario: Diagnostic carries refusal evidence
- **WHEN** acceptance fails due to unparseable output
- **THEN** the diagnostic JSON includes `failureCategory`, `reason`, and a capped excerpt of the offending stdout/stderr.
