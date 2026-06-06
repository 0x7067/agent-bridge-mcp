## MODIFIED Requirements

### Requirement: Tasks expose client-renderable presentation summaries
The system SHALL expose a single lean agent-facing state envelope for Agent Bridge agents
through the consolidated `agent_*` surface so an LLM caller can track an agent without
parsing raw logs, full results, provider prose, or duplicated payloads. Each field appears
once; GUI-only presentation chrome (display title, subtitle, status tone, a structured UI
action-availability array) is not part of the agent-facing envelope.

#### Scenario: Running agent state
- **WHEN** a caller reads `agent_observe` for a running Agent Bridge agent
- **THEN** the response includes `agentId`, `status`, `isFinal: false`, `phase`,
  `progress`, incremental `events`, `nextCursor`, `timedOut`, and a `next` action list
- **AND** each of `progress`, the next-action guidance, and any state object appears exactly
  once in the response.

#### Scenario: Final agent state
- **WHEN** a caller reads `agent_observe` for a final Agent Bridge agent
- **THEN** the response includes the final `status`, `isFinal: true`, `phase`, `progress`,
  and a `next` action list pointing to `agent_result` inspection
- **AND** it does not claim project verification passed.

#### Scenario: Envelope avoids raw payloads and duplication
- **WHEN** a caller reads the default agent state envelope
- **THEN** the response does not include raw stdout, raw stderr, full git diffs, or full
  transcript bodies, and does not repeat next-action, progress, or state objects.

#### Scenario: Opt-in detailed metadata
- **WHEN** a caller reads `agent_observe` or `agent_result` with `verbosity: "detailed"`
- **THEN** the response additionally includes debug metadata such as timestamps, launch
  profile, prompt strategy, and diagnostics
- **AND** the default `verbosity: "compact"` omits that debug metadata.

#### Scenario: Missing agent title
- **WHEN** an agent has no explicit title and a caller requests detailed metadata
- **THEN** any derived display label falls back to a provider and mode label without
  exposing the original prompt body.

### Requirement: Clients can list active and recent agents ergonomically
The system SHALL provide a client-facing way to list active and recent Agent Bridge agents
without requiring clients to process the entire historical registry by default, returning
lean per-agent records rather than GUI presentation blobs.

#### Scenario: Default presentation list
- **WHEN** a client requests the default `agent_list`
- **THEN** the response prioritizes non-final agents first, recent final agents second by
  `updatedAt` descending, excludes removed agents, and includes at most 25 summaries unless
  a smaller limit is requested
- **AND** each record is a lean summary (identity, status, phase, progress, primary `next`
  action) without a GUI action-availability array.

#### Scenario: Filtered presentation list
- **WHEN** a client requests `agent_list` with filters for status, provider, mode,
  workspace, title text, or limit
- **THEN** the response includes only matching agent summaries up to the requested bound and
  rejects limits above 100.

### Requirement: Presentation preserves verification boundaries
The system SHALL keep agent state metadata separate from verification claims about
delegated work.

#### Scenario: Provider reports success
- **WHEN** a provider agent succeeds and the caller reads its final state
- **THEN** the response includes `verificationStatus: "not_verified"` and does not claim
  project tests, lint, typecheck, build, or requested work verification passed.

#### Scenario: Review packet is available
- **WHEN** an agent result includes a review packet
- **THEN** the state metadata may point to the review packet but does not replace raw logs,
  diagnostics, diffs, changed files, or caller-run verification.

## REMOVED Requirements

### Requirement: Presentation summaries expose structured action availability
**Reason**: The structured UI action-availability array (enabled/disabled controls with
state and reason) exists only to let a native GUI client render lifecycle buttons. Agents
are the sole consumers, so it is removed in favor of the single deduplicated `next` action
list defined in `agent-bridge-self-guidance`.
**Migration**: Callers read the top-level `next` array (`{ tool, arguments, safety,
reason }`) instead of `presentation.actions`. Availability is expressed by which actions
appear in `next` and their `safety` classification.

### Requirement: Presentation contract is stable across providers
**Reason**: This requirement guaranteed a single GUI `presentation` summary shape across
providers. With the GUI presentation payload removed, the cross-provider stability
guarantee now applies to the lean agent state envelope and is covered by "Tasks expose
client-renderable presentation summaries" above.
**Migration**: The lean envelope (`agentId`, `status`, `isFinal`, `phase`, `progress`,
`next`) is identical across Claude, Cursor, Kimi, Codex, and Antigravity; provider
differences are expressed through `progress` and which `next` actions are offered.
