# Development Workflows

**Last Updated:** 2026-06-07

Quick reference for common development tasks. Each workflow includes copy-pasteable code examples based on real patterns found in the codebase.

## Quick Reference

| Task | Guide | Time Estimate |
|------|-------|--------------|
| Add a new MCP tool or extend an existing one | [Backend Workflows](backend.md) | ~30 min |
| Add a new provider adapter | [Backend Workflows](backend.md) | ~45 min |
| Write a unit or integration test | [Testing Workflows](unit-tests.md) | ~20 min |
| Compare providers and launch profiles | [Dogfood Provider/Profile Comparison](dogfood-provider-profiles.md) | ~10 min |
| Run quality gates before opening a PR | [Backend Workflows](backend.md) | ~5 min |

## Workflow Guides

- **[Backend Workflows](backend.md)** — How to modify the tool surface, task lifecycle, provider adapters, and server routing
- **[Testing Workflows](unit-tests.md)** — Patterns for deterministic fake-provider tests, PTY tests, and protocol-model tests
- **[Dogfood Provider/Profile Comparison](dogfood-provider-profiles.md)** — Harness for running equivalent read-only prompts through `bridge` and `bare` profiles and capturing transcript/result evidence
