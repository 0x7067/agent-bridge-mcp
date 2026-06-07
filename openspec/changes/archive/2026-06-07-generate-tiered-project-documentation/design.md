## Context

Prior documentation lived in flat files at the repo root (`README.md`, `ARCHITECTURE.md`, `DATA_MODEL.md`, etc.) with no navigational hierarchy. Readers had to scroll through thousands of lines or rely on grep. Agent instructions (`AGENTS.md`, `CLAUDE.md`) were dense walls of text without progressive cues. The cognitive cost was high for both newcomers and returning maintainers.

## Goals / Non-Goals

**Goals:**
- Provide a single front door (`docs/INDEX.md`) that routes readers by intent, not by filename.
- Guarantee that every tiered card has a full-page counterpart for readers who need depth.
- Keep agent instructions (`AGENTS.md`, `CLAUDE.md`) synchronized with the same tiered structure.
- Make documentation age obvious via date stamps.

**Non-Goals:**
- Rewriting content from scratch (we reorganized and polished existing prose, not replaced it).
- Changing the OpenSpec schema or tooling conventions.
- Adding a documentation-generation CI step (generation is manual, triggered by a skill invocation).

## Decisions

### Three tiers instead of two
Considered collapsing into *quick-start* and *deep-dives*, but three tiers maps naturally to the C4 model (system context, containers, component/detail) already used in architecture docs. Third tier holds the codemaps and ADRs that experts visit daily.

### Separate `docs/agents/` terse cards vs. full pages at `docs/`
Terse cards embed bullet tables and tight scoping statements; full pages expand with mermaid diagrams, decision logs, and troubleshooting matrices. Keeping them in parallel directories prevents accidental overwriting and makes link integrity checks easier.

### Front doors hard-link to `docs/INDEX.md`
Both `AGENTS.md` and `CLAUDE.md` replicate the same intro and deep-reference links so that whichever agent loads the repo sees the same progressive disclosure path. They are kept byte-for-byte identical to avoid drift.

### Date stamping in YAML frontmatter-style comments
Instead of hidden metadata, stamps are plain-text annotations (`**Last generated:**`, `**Last verified:**`) so they render in every markdown viewer and act as a visual freshness cue.

## Risks / Trade-offs

- [Rot] Generated docs can go stale if authors forget to regenerate after structural changes. → Mitigation: Regeneration is a single skill invocation (`/skills` → create-documentation); encourage running it after milestone releases.
- [Duplication] Two levels of docs increase total line count. → Mitigation: Cards are deliberately shallow; they do not duplicate paragraphs from full pages, only summarize them.
- [Agent drift] `.cursor/commands/` and `.pi/prompts/` may diverge from `docs/agents/` if updated separately. → Mitigation: Commands and prompts are thin wrappers; heavy content lives in the docs source of truth.

## Migration Plan

Already executed in four commits:
1. Scaffold hierarchical `AGENTS.md` and `CLAUDE.md`.
2. Add `docs/agents/` terse cards and progressive-disclosure links.
3. Generate full tiered documentation set (`docs/TIER-X/` and supporting pages).
4. Consolidate `AGENTS.md` with the final progressive-disclosure polish and date stamps.

Rollback: Delete `docs/agents/` and revert `AGENTS.md`/`CLAUDE.md` to pre-tier versions.

## Open Questions

- Should we enforce a CI check that warns when `docs/INDEX.md` links to missing files? (Deferred; not currently gated.)
- Should agent prompts ingest the terse cards dynamically instead of embedding static summaries? (Deferred to future ergonomic work.)
