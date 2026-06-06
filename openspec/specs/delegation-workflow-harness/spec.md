# delegation-workflow-harness Specification

## Purpose
Define the operational workflow for using delegated provider tasks safely in real MCP clients, including readiness checks, previews, spawning, waiting, log inspection, result review, cleanup, and optional live smoke probes.
## Requirements
### Requirement: Workspace confinement uses workspace path-list
The system SHALL confine task `cwd` values using `AGENT_BRIDGE_WORKSPACES`, a platform path-list of allowed workspace roots.

#### Scenario: Cwd is inside one configured workspace
- **WHEN** `AGENT_BRIDGE_WORKSPACES` contains multiple workspace roots and a caller supplies a `cwd` inside one of them
- **THEN** the task preview or spawn request is accepted after canonicalizing the workspace root and `cwd`

#### Scenario: Cwd is outside configured workspaces
- **WHEN** a caller supplies a `cwd` outside every configured workspace root
- **THEN** the task preview or spawn request is rejected with an outside-workspace error

#### Scenario: Legacy allowed root variable is not used
- **WHEN** `AGENT_BRIDGE_ALLOWED_ROOT` is set but `AGENT_BRIDGE_WORKSPACES` is unset
- **THEN** the server does not use `AGENT_BRIDGE_ALLOWED_ROOT` for workspace confinement

### Requirement: Delegation workflow is documented
The system SHALL document the standard caller workflow for using provider tasks from a real MCP host.

#### Scenario: Standard lifecycle is discoverable
- **WHEN** an operator reads the project documentation
- **THEN** the documentation describes provider readiness checks, task preview, task spawn, bounded wait, incremental logs, final result inspection, and explicit cleanup

#### Scenario: Stalled task guidance is documented
- **WHEN** a provider task appears stalled or exceeds a short wait
- **THEN** the documentation explains how to inspect incremental logs, stop the task, and inspect the stopped result

### Requirement: Live smoke workflow is opt-in
The system SHALL define an intentional live-smoke workflow for installed provider CLIs without making live provider execution part of the default CI suite.

#### Scenario: Operator runs live provider readiness smoke
- **WHEN** an operator chooses to run live smoke checks
- **THEN** the workflow runs `providers_check` with `smoke: true` using bounded timeouts and reports provider diagnostics

#### Scenario: Default verification remains deterministic
- **WHEN** the default automated test suite runs
- **THEN** it does not require live provider credentials, paid model access, network access, or host-specific keychain permissions

### Requirement: Delegated implementation uses inspectable isolation by default
The system SHALL document managed worktree isolation as the default workflow for write-capable delegated tasks unless a caller intentionally selects another isolation mode.

#### Scenario: Implementation task guidance
- **WHEN** the documentation shows or describes a provider task in `implement` mode
- **THEN** it recommends `isolation: "worktree"` so the main thread can inspect changes before integration

#### Scenario: Cleanup remains explicit after inspection
- **WHEN** a managed worktree task reaches a final state
- **THEN** the workflow requires result inspection before calling `agent_remove` to clean the task record and managed worktree

### Requirement: Delegated results are treated as evidence
The system SHALL document that provider task results are evidence for the main thread rather than final verification by themselves.

#### Scenario: Provider reports success
- **WHEN** a provider task reports success or returns a final result
- **THEN** the workflow still requires the main thread to inspect output, review diffs when present, and run the relevant project verification gates before claiming completion

#### Scenario: Provider result is incomplete or risky
- **WHEN** a provider result is ambiguous, failing, stale, or changes unexpected files
- **THEN** the workflow directs the main thread to stop, inspect, discard, or re-run the task instead of automatically integrating the result

### Requirement: Delegation workflow documents task presentation
The system SHALL document how clients should use Agent Bridge `presentation` metadata on task lifecycle responses to render delegated provider tasks as native-feeling agents while preserving the raw lifecycle workflow for automation and debugging.

#### Scenario: Native-client workflow is discoverable
- **WHEN** an operator reads project documentation or MCP guidance
- **THEN** the guidance describes the native-client path for listing task presentation summaries, inspecting status, reading results, stopping running tasks, and cleaning up final tasks.

#### Scenario: Raw lifecycle workflow remains documented
- **WHEN** an operator reads project documentation or MCP guidance
- **THEN** the guidance still documents the lower-level preview, spawn, wait, logs, transcript, result, stop, and remove workflow.

### Requirement: Delegation workflow explains unavailable interactive controls
The system SHALL document how clients should present reply and resume controls for providers or tasks that do not support interactive continuation.

#### Scenario: Reply is unavailable
- **WHEN** provider capability metadata or task action availability reports reply as unsupported
- **THEN** workflow guidance tells clients to present the action with `state: "unavailable"` and an explanation rather than treating it as a failed tool call.

#### Scenario: Resume is unavailable
- **WHEN** provider capability metadata or task action availability reports resume as unsupported
- **THEN** workflow guidance tells clients to present the action with `state: "unavailable"` and an explanation and to use a new task when continuation is required.

### Requirement: Delegation workflow prioritizes active and recent tasks
The system SHALL document that native clients should prioritize active and recent Agent Bridge tasks over the full historical registry.

#### Scenario: Client opens agent drawer
- **WHEN** a client opens a native Agent Bridge agent list or drawer
- **THEN** workflow guidance directs it to show active tasks and recent final tasks first.

#### Scenario: Operator needs historical records
- **WHEN** an operator needs older completed task records
- **THEN** workflow guidance points to filtered or raw registry inspection rather than defaulting the native presentation to all historical tasks.

