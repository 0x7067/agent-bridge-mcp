## 1. Front Door Instructions

- [x] 1.1 Draft progressive-disclosure `AGENTS.md` with tiered deep-links and hard-link front door.
- [x] 1.2 Mirror identical structure into `CLAUDE.md` for agent consumption parity.
- [x] 1.3 Verify `AGENTS.md` and `CLAUDE.md` are byte-aligned on key sections.

## 2. Terse Agent Cards

- [x] 2.1 Create `docs/agents/getting-started.md` — clone, build, run, first PR, env setup.
- [x] 2.2 Create `docs/agents/architecture.md` — module map, eight-tool surface, boundaries.
- [x] 2.3 Create `docs/agents/data-model.md` — registry, task states, filesystem layout.
- [x] 2.4 Create `docs/agents/security.md` — threat model, workspace confinement, secrets.
- [x] 2.5 Create `docs/agents/guardrails.md` — PTY hazards, MCP protocol contract, secrets rule.
- [x] 2.6 Create `docs/agents/definition-of-done.md` — `scripts/quality.sh` gates, test notes.
- [x] 2.7 Create `docs/agents/tooling.md` — workspace structure, deps, one-time installs.

## 3. Full Guides & Diagrams

- [x] 3.1 Generate `docs/GETTING-STARTED.md` with prerequisites, install, run, first PR.
- [x] 3.2 Generate `docs/SETUP.md` with full environment walkthrough and troubleshooting.
- [x] 3.3 Generate `docs/ARCHITECTURE/system-context.md` and `containers.md` (C4 Level 1/2).
- [x] 3.4 Generate `docs/DATA-MODEL.md` with ER diagram, lifecycle state machine.
- [x] 3.5 Generate `docs/SECURITY.md` with roles, permissions, isolation, threat notes.
- [x] 3.6 Generate `docs/INTEGRATIONS.md` with third-party connections.
- [x] 3.7 Generate `docs/CODEMAPS/backend.md`, `integrations.md`, `state-store.md` with module heatmaps.
- [x] 3.8 Generate `docs/WORKFLOWS/backend.md` and `unit-tests.md` with daily developer patterns.

## 4. Navigation & Freshness

- [x] 4.1 Generate `docs/INDEX.md` as the progressive-disclosure manifest with tiered tables.
- [x] 4.2 Add `Last generated` and `Last verified` date stamps to every produced document.
- [x] 4.3 Ensure every terse card links outward to its full-page counterpart.
- [x] 4.4 Ensure `.cursor/commands/` and `.pi/prompts/` wrap the same guidance without duplicating prose.

## 5. Validation

- [x] 5.1 Run `markdownlint` or manual scan for broken internal links.
- [x] 5.2 Visually verify `docs/INDEX.md` renders correctly in a markdown viewer.
- [x] 5.3 Spot-check that `AGENTS.md` → `docs/agents/architecture.md` → `docs/ARCHITECTURE/containers.md` chain works.
- [x] 5.4 Confirm no stale references to pre-tier file names (e.g., `ARCHITECTURE.md` at root).
