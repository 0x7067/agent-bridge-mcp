## MODIFIED Requirements

### Requirement: Task results include a delegated review packet
The system SHALL return a `reviewPacket` summary from `agent_result` and SHALL let callers
select which evidence sections to include so that large raw evidence is fetched on demand
rather than returned by default. `agent_result` is the single tool for both summarized and
raw provider evidence, subsuming standalone log inspection.

#### Scenario: Default compact packet
- **WHEN** a caller reads `agent_result` without a `sections` argument
- **THEN** the response includes the `reviewPacket` summary and `changedFiles`
- **AND** it does not inline full stdout, full stderr, full git diff, or full transcript
  bodies.

#### Scenario: Requested evidence sections
- **WHEN** a caller reads `agent_result` with `sections` including `stdout`, `stderr`,
  `diff`, or `transcript`
- **THEN** the response includes the requested evidence bounded by caps and the
  `maxBytes`/`stdoutLine`/`stderrLine`/`cursor` pagination controls
- **AND** truncation flags indicate when a section was capped.

#### Scenario: Raw log inspection through result sections
- **WHEN** a caller needs incremental stdout and stderr (the former `agent_logs` use case)
- **THEN** the caller reads `agent_result` with `sections: ["stdout","stderr"]` and line
  pagination rather than a separate log tool.

#### Scenario: Successful task with no repository changes
- **WHEN** a caller reads `agent_result` for a successful task whose git status and changed
  files are empty
- **THEN** the packet includes `status`, `isFinal`, `hasChanges: false`, `changedFiles: []`,
  truncation flags, exit metadata, and recommended actions that tell the caller to inspect
  provider output and run relevant verification before claiming completion.

#### Scenario: Failed task result
- **WHEN** a caller reads `agent_result` for a failed task
- **THEN** the packet includes `errorType`, exit metadata, diagnostic data when available,
  and recommended actions that point the caller to evidence sections, diagnostics, and
  rerun or manual recovery decisions.

#### Scenario: Managed worktree result
- **WHEN** a caller reads `agent_result` for a task that used managed worktree isolation
- **THEN** recommended actions include calling `agent_remove` only after the managed
  worktree result has been inspected.
