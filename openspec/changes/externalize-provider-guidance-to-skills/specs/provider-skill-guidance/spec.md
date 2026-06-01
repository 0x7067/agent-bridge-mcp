## ADDED Requirements

### Requirement: Repository provides provider skill guidance
The system SHALL provide repo-owned agent skill guidance for every first-class Agent Bridge provider.

#### Scenario: Provider skills cover all first-class providers
- **WHEN** provider skill guidance is validated
- **THEN** the repository includes one provider skill for each supported provider: `claude`, `codex`, `cursor`, and `kimi`

#### Scenario: Provider skill source is repo-owned
- **WHEN** provider skill guidance is validated
- **THEN** validation reads the repo-owned skill source files and does not depend on personal global skill installs such as `~/.claude/skills`

### Requirement: Provider skills define a stable runbook contract
Each provider skill SHALL include enough structured content for agents to use the provider CLI directly without relying on Agent Bridge runtime internals.

#### Scenario: Required skill metadata is present
- **WHEN** a provider skill file is inspected
- **THEN** it includes YAML frontmatter with `name`, `description`, `provider_id`, `provider_cli`, and `supported_modes`

#### Scenario: Pi agent skill documents Kimi provider
- **WHEN** the `pi-agent` provider skill is inspected
- **THEN** its frontmatter uses `name: pi-agent`, `provider_id: kimi`, `provider_cli: pi`, and a non-empty `pinned_model`

#### Scenario: Pi agent pins Kimi model
- **WHEN** the `pi-agent` default direct invocation is inspected
- **THEN** it invokes `pi` with `--model` set to the skill's pinned Kimi model

#### Scenario: Required operational sections are present
- **WHEN** a provider skill file is inspected
- **THEN** it documents install/version verification, default safe invocation, write-capable or auto-approval flags, safety constraints, output evidence expectations, and provider-specific troubleshooting notes

#### Scenario: Agent Bridge modes are mapped
- **WHEN** a provider skill documents direct CLI usage
- **THEN** it maps direct invocation guidance to Agent Bridge task modes where the mapping is meaningful

### Requirement: Provider skills separate direct invocation from MCP delegation
Provider skills SHALL explain direct provider CLI usage while preserving Agent Bridge as the owner of MCP-native task orchestration.

#### Scenario: Direct CLI task
- **WHEN** an operator wants a one-shot provider CLI invocation outside Agent Bridge
- **THEN** the relevant provider skill describes the direct command shape, safe default flags, and evidence to inspect after the process exits

#### Scenario: MCP task lifecycle task
- **WHEN** an operator needs background execution, managed worktree isolation, readiness checks, log polling, diff inspection, or final task result metadata
- **THEN** the provider skill points the operator back to Agent Bridge lifecycle tools instead of reimplementing that workflow

#### Scenario: Runtime does not parse skill prose
- **WHEN** Agent Bridge builds or previews a provider task command
- **THEN** command construction uses provider adapter runtime metadata rather than reading provider skill markdown

### Requirement: Provider skill validation prevents drift
The system SHALL validate that provider skill guidance remains aligned with the provider runtime metadata exposed by Agent Bridge.

#### Scenario: Provider names match runtime metadata
- **WHEN** validation compares provider skill metadata with runtime provider metadata
- **THEN** every skill `provider_id` matches a provider exposed by `providers_list`

#### Scenario: Skill name may differ from provider id
- **WHEN** validation inspects the `pi-agent` skill
- **THEN** it accepts the skill name differing from `provider_id` because `provider_id: kimi` links the skill to the runtime provider

#### Scenario: Supported modes match runtime metadata
- **WHEN** validation compares provider skill metadata with runtime provider metadata
- **THEN** each skill's `supported_modes` match the modes exposed for that provider by `providers_list`

#### Scenario: Missing or duplicate provider skill fails validation
- **WHEN** a supported provider has no provider skill or more than one provider skill
- **THEN** the default validation suite fails with an actionable error naming the affected provider

#### Scenario: Validation remains deterministic
- **WHEN** the default validation suite runs
- **THEN** provider skill validation does not require live provider credentials, network access, or personal host-specific skill directories

### Requirement: Provider skills preserve safety warnings for dangerous flags
Provider skills SHALL clearly identify flags or modes that grant unattended write, shell, auto-approval, or broad filesystem access.

#### Scenario: Dangerous flag is documented
- **WHEN** a provider skill mentions a dangerous flag or write-capable mode
- **THEN** it states that the flag or mode requires explicit user authorization before use

#### Scenario: Safe default is documented first
- **WHEN** a provider skill documents invocation commands
- **THEN** the default command is the safest useful direct invocation for that provider before any write-capable or auto-approval variant
