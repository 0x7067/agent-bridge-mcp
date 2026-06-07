## ADDED Requirements

### Requirement: Published documentation is organized into three tiers
All published documentation SHALL be reachable through a tiered progression:

- **Tier 1 — Survival**: Terse cards under `docs/agents/` providing enough information to clone, build, run, and make a first contribution without reading full pages.
- **Tier 2 — Comprehension**: Full guides under `docs/` explaining architectural rationale, data models, and operational context.
- **Tier 3 — Mastery**: Deep reference under `docs/CODEMAPS/`, `docs/WORKFLOWS/`, and `docs/ADR/` for daily development and archaeological debugging.

#### Scenario: New contributor onboarding
- **WHEN** a new contributor opens the repository
- **THEN** they can read `README.md`, navigate to `docs/INDEX.md`, choose Tier 1
- **AND** within minutes know how to build and test the project without scrolling through thousands of lines.

#### Scenario: Expert archaeologist
- **WHEN** an expert needs to understand why a module boundary exists
- **THEN** they navigate from `docs/INDEX.md` → Tier 3 → `docs/ADR/0003-decompose-task-module.md`
- **AND** find the decision rationale.

### Requirement: The navigation manifest enumerates every document by tier
`docs/INDEX.md` SHALL enumerate every document by tier and purpose, acting as the sole front door for human readers.

#### Scenario: Discovering the security guide
- **WHEN** a reader wants to understand the threat model
- **THEN** `docs/INDEX.md` lists `[Agents: Security](agents/security.md)` under Tier 1
- **AND** `[Security Patterns](SECURITY.md)` under Tier 2
- **AND** the reader can pick the depth they need.

### Requirement: Agent instructions mirror the tiered structure
`AGENTS.md` and `CLAUDE.md` SHALL present the same tiered deep-link structure as `docs/INDEX.md`, ensuring that any agent loading the repository receives identical progressive disclosure.

#### Scenario: Agent-assisted code review
- **WHEN** an agent is asked to review a PR
- **THEN** it loads `AGENTS.md`
- **AND** immediately sees the same tiered pointers a human would, routing to `docs/agents/definition-of-done.md` for validation rules.

### Requirement: Generated documents carry freshness stamps
Every generated document MUST include a visible `Last generated:` or `Last verified:` stamp so documentation age is apparent at a glance.

#### Scenario: Monthly health check
- **WHEN** a maintainer reviews `docs/INDEX.md`
- **THEN** they notice a Tier 2 page was last verified three months ago
- **AND** they trigger regeneration to refresh screenshots and env variable references.

### Requirement: Tiered cards link bidirectionally to full pages
Each Tier 1 terse card MUST link to its Tier 2 full-page counterpart. Each Tier 2 page MAY link onward to Tier 3 deep reference.

#### Scenario: Drilling down from quick reference
- **WHEN** a developer skimming `docs/agents/architecture.md` needs container-level detail
- **THEN** they click the outbound link to `docs/ARCHITECTURE/containers.md`.

## ACCEPTANCE CRITERIA

- `docs/INDEX.md` renders without broken internal links.
- `AGENTS.md` and `CLAUDE.md` contain identical tiered introductory sections.
- Every `docs/agents/*.md` file exists and is ≤ 400 lines.
- No root-level documentation files (excluding `README.md` and `LICENSE`) duplicate Tier 2 content.
