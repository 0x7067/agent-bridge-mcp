## ADDED Requirements

### Requirement: MCP task support is compatibility-gated
The system SHALL require an explicit compatibility decision before advertising or implementing protocol-level MCP task support.

#### Scenario: Compatibility memo precedes task capability advertisement
- **WHEN** MCP task support implementation begins
- **THEN** the change includes a documented compatibility decision naming the targeted MCP version, capability shape, supported methods, and unsupported methods.

#### Scenario: No unsupported task claims
- **WHEN** a client sends `initialize` before MCP task compatibility is implemented
- **THEN** Agent Bridge does not advertise task capabilities it does not implement.

### Requirement: MCP task state maps from Agent Bridge task records
The system SHALL derive protocol task state from existing Agent Bridge task records rather than maintaining a separate task registry.

#### Scenario: Running task status mapping
- **WHEN** an Agent Bridge task is queued or running
- **THEN** the protocol task status maps to a non-terminal working state.

#### Scenario: Final task status mapping
- **WHEN** an Agent Bridge task is succeeded, failed, stopped, or stale
- **THEN** the protocol task status maps to a terminal completed, failed, or cancelled state according to the compatibility design.

#### Scenario: Task timestamps are derived
- **WHEN** Agent Bridge returns protocol task state
- **THEN** created and last-updated timestamps are derived from the task record.

### Requirement: MCP task cancellation preserves inspectability
The system SHALL map protocol task cancellation to stopping execution without automatically removing inspectable task artifacts.

#### Scenario: Cancel running task
- **WHEN** a client cancels a protocol task mapped to a running Agent Bridge task
- **THEN** Agent Bridge attempts to stop the provider process and records a final cancelled or stopped-equivalent state.

#### Scenario: Cancel does not remove worktree
- **WHEN** a cancelled task has a managed worktree or logs
- **THEN** Agent Bridge keeps the task inspectable through existing result/log surfaces until explicit cleanup.

#### Scenario: Cancel final task is rejected
- **WHEN** a client attempts to cancel a task already in a terminal mapped state
- **THEN** Agent Bridge rejects the cancellation using the protocol error semantics selected by the compatibility design.

### Requirement: MCP task progress and status notifications remain optional
The system SHALL not require task status or progress notifications for correctness.

#### Scenario: Polling works without notifications
- **WHEN** a client supports MCP task polling but does not consume status notifications
- **THEN** the client can still observe task completion through the supported polling/result methods.

#### Scenario: Notifications follow negotiated support
- **WHEN** Agent Bridge emits protocol task status or progress notifications
- **THEN** it only emits notifications allowed by the negotiated protocol and client capabilities.

### Requirement: Existing Agent Bridge lifecycle remains primary and stable
The system SHALL preserve the existing Agent Bridge `task_*` lifecycle tools regardless of MCP task support.

#### Scenario: Legacy lifecycle remains available
- **WHEN** a client does not advertise MCP task support
- **THEN** the existing `task_*` tools continue to provide spawn, list, status, wait, logs, transcript, result, stop, and cleanup behavior.

#### Scenario: MCP task support is additive
- **WHEN** a client advertises supported MCP task capabilities
- **THEN** Agent Bridge task support is additive and does not remove existing lifecycle tool behavior.
