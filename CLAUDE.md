# Agent Bridge MCP

Rust stdio MCP server that delegates bounded work from one agent client to local
provider agents (Claude Code, Codex, Cursor, Kimi/Pi, Forge, Antigravity). Exposes a
lean eight-tool lifecycle; the calling agent stays responsible for verification.

## Tech Stack

- Rust 2024 edition, single-crate workspace (`crates/agent-bridge-mcp`)
- Tokio async runtime; `pty-process` for interactive provider PTYs
- serde / serde_json (MCP JSON-RPC over stdio), chrono, uuid, libc
- CI quality gate: rustfmt, clippy `-D warnings`, cargo-machete, jscpd (<5%)

## Key Rules

- Provider/subagent output is **evidence, not proof** — verify locally before claiming done.
- Clippy runs with `-D warnings`: warnings fail CI. Zero tolerance, including pre-existing.
- jscpd fails over 5% duplication — extract shared helpers, don't copy-paste.
- Don't add dependencies casually; cargo-machete fails on unused ones.
- PTY/interactive tests touch global process state — see guardrails before changing them.

## Workflow

Every task follows four stages. Identify which stage you're in and follow its rules.

```
Plan → Execute → Validate → Commit
 ↑                            |
 └── fix ─────────────────────┘
```

1. **Plan** — Understand the task, research code, design approach. List unresolved questions.
2. **Execute** — Implement changes AND tests together. No implementation is complete without tests.
3. **Validate** — ALL checks pass with zero errors before moving on. Run `scripts/quality.sh`.
   If ANY check fails → return to Execute, fix, re-validate. Pre-existing errors are NOT exempt.
4. **Commit** — Only after Validate passes. Conventional Commits, atomic. Never push unless asked.

## Detailed Guidance

When working on tasks involving these topics, read the linked doc:

- **Getting started** (`docs/agents/getting-started.md`) — clone, build, run, first PR, environment setup
- **Tooling & build** (`docs/agents/tooling.md`) — cargo workspace, the two binaries, rustfmt/clippy/machete config
- **Validation** (`docs/agents/definition-of-done.md`) — `scripts/quality.sh`, the exact gates and thresholds, running tests
- **Architecture** (`docs/agents/architecture.md`) — module map, the eight MCP tools, provider/task/claude_interactive boundaries
- **Security** (`docs/agents/security.md`) — threat model, workspace confinement, secret hygiene, isolation
- **Guardrails** (`docs/agents/guardrails.md`) — PTY/global-state test hazards, MCP protocol contract, secrets
- **Deep reference** (`docs/INDEX.md`) — full documentation manifest: data model, business context, ADRs, codemaps, workflows
- Run `/skills` to see available patterns and workflows
