# acp-router-contract Specification

## Purpose

Define the ACP-facing Agent Bridge router contract: one routed prompt turn in,
one final provider-authored answer, blocker, or classified failure out, with
diagnostic evidence preserved by reference.

## Requirements

### Requirement: Router exposes a prompt-turn ACP contract

The system SHALL expose an explicit ACP router runtime path that models one
routed prompt turn instead of caller-managed provider task lifecycle.

#### Scenario: ACP router initializes
- **WHEN** an ACP client sends `initialize` to the router runtime
- **THEN** the router returns ACP-compatible capabilities for routed prompt turns.
- **AND** it does not advertise the MCP lifecycle tool list.

#### Scenario: Router creates a session
- **WHEN** an ACP client sends `session/new`
- **THEN** the router creates a router session without launching a provider.

#### Scenario: Router handles one prompt turn
- **WHEN** an ACP client sends `session/prompt`
- **THEN** the router chooses a provider, executes the prompt turn, and returns
  exactly one terminal outcome for that prompt.

### Requirement: Router v1 routes only Codex and Claude

The system SHALL limit router v1 provider candidates to Codex and Claude while
leaving lower-level MCP provider adapters unchanged for compatibility.

#### Scenario: Candidate set includes supported router providers
- **WHEN** policy inputs select Codex, Claude, or both
- **THEN** the router may consider those providers according to readiness and
  policy order.

#### Scenario: Candidate set includes unsupported router provider
- **WHEN** policy inputs select Cursor, Kimi, Forge, Antigravity, or an unknown
  provider for router execution
- **THEN** the router rejects the request before launching any provider attempt.

### Requirement: Router returns compact terminal results

The system SHALL return a compact routed-turn result containing selected provider,
terminal kind, final text or blocker/failure message, attempt metadata, evidence
references, and bounded diagnostics.

#### Scenario: Provider returns final answer
- **WHEN** the selected provider produces provider-authored final text
- **THEN** the router returns that final text as the routed-turn answer.
- **AND** the router treats it as trusted finality for the prompt turn.

#### Scenario: Provider returns blocker
- **WHEN** the selected provider produces a refusal, cancellation, auth blocker,
  billing blocker, or equivalent terminal semantic blocker
- **THEN** the router returns a blocker outcome with the classified reason.

#### Scenario: Provider fails without finality
- **WHEN** the selected provider cannot produce a trusted final answer or blocker
- **THEN** the router returns a classified failure if no failover succeeds.

### Requirement: Router preserves evidence without raw lifecycle by default

The system SHALL preserve transcripts, stdout/stderr excerpts, provider diagnostics,
and task review evidence while keeping raw lifecycle bodies out of the default
router result.

#### Scenario: Compact result references evidence
- **WHEN** a routed turn completes
- **THEN** the router result includes evidence references sufficient to inspect
  the underlying attempt artifacts.
- **AND** it does not embed raw stdout, stderr, transcript, or diff bodies by
  default.

#### Scenario: Provider internals stream as debug evidence
- **WHEN** a provider emits progress, thought, lifecycle, or transcript events
  during a routed prompt turn
- **THEN** the router may emit bounded ACP `session/update` evidence/debug events.
- **AND** those events do not become the terminal product contract.

### Requirement: MCP lifecycle remains migration compatibility

The system SHALL preserve the existing eight-tool MCP lifecycle behavior during
router migration, but it SHALL NOT define the final replacement product contract.

#### Scenario: Default runtime remains MCP-compatible
- **WHEN** the binary runs without the ACP router runtime path
- **THEN** existing MCP initialization, tools, and lifecycle behavior remain
  compatible.

#### Scenario: Router runtime is selected explicitly
- **WHEN** the binary runs in ACP router mode
- **THEN** it handles the router ACP contract instead of the MCP tool contract.
