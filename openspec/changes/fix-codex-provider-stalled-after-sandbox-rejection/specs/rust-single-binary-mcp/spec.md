## ADDED Requirements

### Requirement: Provider fatal errors finalize task lifecycle
The Rust MCP binary SHALL transition tasks to a final failed state when a provider emits unrecoverable fatal-error evidence, even if the provider process does not exit promptly.

#### Scenario: Fatal provider stderr is captured
- **WHEN** a provider emits fatal-error evidence on stderr
- **THEN** the Rust binary captures the stderr in task logs and diagnostic excerpts without writing non-MCP bytes to stdout

#### Scenario: Fatal provider evidence ends running state
- **WHEN** a running task has provider fatal-error evidence that cannot recover
- **THEN** lifecycle tools stop reporting the task as running after a bounded cleanup period
- **AND** the task remains inspectable through `task_result`

#### Scenario: Fatal provider cleanup terminates process tree
- **WHEN** Agent Bridge finalizes a task early because of fatal provider evidence
- **THEN** it terminates and reaps the provider process group or child tree before recording the final state

#### Scenario: Fatal provider finalization preserves existing timeout behavior
- **WHEN** a provider does not emit known fatal-error evidence and exceeds its configured timeout
- **THEN** the Rust binary preserves existing timeout classification and diagnostics
