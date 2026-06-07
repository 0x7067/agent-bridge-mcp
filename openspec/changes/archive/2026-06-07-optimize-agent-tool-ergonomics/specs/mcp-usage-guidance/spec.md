## MODIFIED Requirements

### Requirement: Guidance mirrors initialization instructions
The system SHALL keep MCP prompts and resources aligned with the initialization
instructions for the standard Agent Bridge workflow over the consolidated eight-tool
surface.

#### Scenario: Caller workflow guidance names self-guided surfaces
- **WHEN** a client reads caller workflow guidance
- **THEN** the guidance mentions initialization instructions, structured tool results, the
  single `next` action metadata, and the consolidated lifecycle tools.

#### Scenario: Guidance preserves fallback path
- **WHEN** a client does not use initialization instructions or structured content
- **THEN** prompts and resources still describe the manual lifecycle using `doctor`,
  `agent_spawn`, `agent_observe`, `agent_result`, `agent_stop`, and `agent_remove`, and
  describe the subsuming parameters (`dryRun`, `until`, `sections`, `focus`) rather than
  removed tools.

### Requirement: Guidance explains Codex sandbox denial recovery
The system SHALL document how callers should investigate and recover from Codex sandbox,
approval, or out-of-workspace patch denials using the consolidated surface.

#### Scenario: Guidance names Codex denial symptoms
- **WHEN** a client reads Agent Bridge recovery, safety, or provider guidance
- **THEN** the guidance mentions Codex patch rejection, sandbox denial, approval denial, or
  out-of-workspace write symptoms as setup or prompt-scope issues to inspect.

#### Scenario: Guidance recommends bounded lifecycle inspection
- **WHEN** guidance describes recovering from Codex denial failures
- **THEN** it tells callers to use bounded `agent_observe` (including `until: "final"`) and
  final `agent_result` evidence inspection instead of waiting indefinitely.

#### Scenario: Guidance preserves safety boundary
- **WHEN** guidance describes follow-up actions for Codex denial failures
- **THEN** it tells callers to inspect `cwd`, workspace policy, prompt scope, and isolation
  strategy before retrying
- **AND** it does not tell callers to silently relax sandbox permissions.

## ADDED Requirements

### Requirement: Guidance teaches code-execution-friendly delegation
The system SHALL expose guidance describing how to drive Agent Bridge with minimal context
cost, suitable for code-execution and Tool-Search-style callers.

#### Scenario: Code-execution guidance resource exists
- **WHEN** a client sends `resources/list`
- **THEN** the response includes an `agent-bridge://guidance/code-execution` resource.

#### Scenario: Code-execution guidance content
- **WHEN** a client reads the code-execution guidance resource
- **THEN** the markdown explains polling compactly with `agent_observe`
  (`until`/`timeoutMs`), fetching evidence sections on demand from `agent_result`, keeping
  raw logs and diffs out of context until needed, and running caller-owned verification
  before claiming completion.
