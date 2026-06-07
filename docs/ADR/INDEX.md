# Architecture Decision Records

This directory contains Architecture Decision Records (ADRs) for Agent Bridge MCP. ADRs capture the context, decision, and consequences of architecturally significant choices.

**Format:** [MADR 4.0.0](https://adr.github.io/madr/) (Markdown Architectural Decision Records)

## How to Read These

- Start with the ADRs marked as `accepted` — these represent the current architecture
- ADRs marked `reconstructed` were generated from git history analysis — their context section may be incomplete

## Decision Log

| ADR | Date | Status | Decision |
|-----|------|--------|----------|
| [ADR-0001](0001-consolidate-eight-tools.md) | 2025-03-?? | accepted | Consolidate 14 tools into 8 with lean responses |
| [ADR-0002](0002-harden-task-lifecycle.md) | 2025-04-?? | accepted | Harden task lifecycle and extract provider adapter abstraction |
| [ADR-0003](0003-decompose-task-module.md) | 2025-04-?? | accepted | Decompose monolithic task.rs into five focused submodules |
| [ADR-0004](0004-static-intelligence-gates.md) | 2025-04-?? | accepted | Add static codebase intelligence gates (complexity, duplication, cycles) |
| [ADR-0005](0005-claude-pty-host-runner.md) | 2025-02-?? | accepted | Route Claude through an owned PTY host runner via Unix socket |
