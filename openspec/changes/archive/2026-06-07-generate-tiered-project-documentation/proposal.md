## Why

Project documentation had grown organically into a flat, inconsistent mix of full-length guides, terse reference cards, and scattered markdown files. New contributors faced a steep wall of text; experienced developers struggled to find quick reference material. We needed a predictable, progressive disclosure structure so readers could start with survival-grade brevity and drill down to mastery depth only when necessary.

## What Changes

- Introduce a three-tier documentation taxonomy:
  - **Tier 1 — Survival**: Terse `docs/agents/` cards (getting-started, architecture, data-model, security, guardrails, definition-of-done, tooling). Enough to clone, build, run, and contribute safely.
  - **Tier 2 — Comprehension**: Full-page guides (`docs/GETTING-STARTED.md`, `docs/SETUP.md`, `docs/ARCHITECTURE/`, `docs/DATA-MODEL.md`, `docs/SECURITY.md`, `docs/INTEGRATIONS.md`). Explains why the system works this way.
  - **Tier 3 — Mastery**: Deep reference (`docs/DOCUMENTATION.md`, `docs/CODEMAPS/`, `docs/WORKFLOWS/`, `docs/ADR/`). Daily-use codemaps, decision records, and backend workflows.
- Generate `docs/INDEX.md` as the progressive-disclosure front door, linking each card/page by tier and purpose.
- Restructure `AGENTS.md` and `CLAUDE.md` as hard-linked front-door agent instructions with the same survival/comprehension/mastery pointers.
- Stamp every generated doc with `Last generated:` and `Last verified:` dates so rot is detectable.
- Mirror the same tiered philosophy into `.cursor/commands/` and `.pi/prompts/` so every agent interface presents the same graduated guidance.

## Capabilities

### New Capabilities
- `documentation-structure`: Defines the tiered layout, naming conventions, and generation rules for Agent Bridge documentation sets.

### Modified Capabilities
*(No spec-level behavioral changes to MCP tools or runtime contracts.)*

## Impact

- Purely additive to markdown artifacts; no functional code changes.
- Touches `docs/**`, `AGENTS.md`, `CLAUDE.md`, `.cursor/`, `.pi/prompts/`.
- Establishes a repeatable pattern for future doc sweeps.
