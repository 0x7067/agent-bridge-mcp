# Development Workflows

**Last Updated:** 2026-06-20

Quick reference for common development tasks. Each workflow includes copy-pasteable code examples based on real patterns found in the codebase.

## Quick Reference

| Task | Guide | Time Estimate |
|------|-------|--------------|
| Change ACP or MCP adapter behavior | [Backend Workflows](backend.md) | ~30 min |
| Add a new provider adapter | [Backend Workflows](backend.md) | ~45 min |
| Write a unit or integration test | [Testing Workflows](unit-tests.md) | ~20 min |
| Read archived provider/profile comparison notes | [Archived Dogfood Provider/Profile Comparison](dogfood-provider-profiles.md) | ~5 min |
| Run quality gates before opening a PR | [Backend Workflows](backend.md) | ~5 min |

## Workflow Guides

- **[Backend Workflows](backend.md)** — How to modify ACP routing, the two-tool MCP adapter, provider adapters, and internal task lifecycle behavior
- **[Testing Workflows](unit-tests.md)** — Patterns for deterministic fake-provider tests, PTY tests, and protocol-model tests
- **[Archived Dogfood Provider/Profile Comparison](dogfood-provider-profiles.md)** — Historical notes for the removed lifecycle-tool comparison harness
