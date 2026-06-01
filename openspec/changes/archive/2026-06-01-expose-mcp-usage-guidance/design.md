## Context

Agent Bridge currently exposes a provider-neutral task lifecycle through MCP tools and documents the caller workflow in `README.md`. MCP clients discover the tools at runtime, but they do not receive the longer workflow, safety, and provider guidance unless the user or agent reads repository docs separately.

The server currently advertises protocol version `2024-11-05`. That revision defines prompts and resources, but not the newer `initialize.instructions` field. This change therefore uses prompts and resources for runtime guidance while keeping protocol version compatibility unchanged.

## Goals / Non-Goals

**Goals:**
- Expose concise, MCP-native usage guidance through static prompt templates and readable resources.
- Advertise `prompts` and `resources` capabilities during initialization.
- Keep resource reads safe by validating every resource URI against a hardcoded allowlist.
- Preserve existing task lifecycle tool names, arguments, and provider behavior.
- Document that client handling of prompts/resources is client-dependent.

**Non-Goals:**
- Do not change provider command construction, task execution, worktree cleanup, or state persistence.
- Do not auto-spawn tasks or make provider delegation mandatory.
- Do not upgrade the server protocol version or add `initialize.instructions` until explicit version negotiation is designed.
- Do not read arbitrary local files through `resources/read`.

## Decisions

### Use prompts for workflow templates

The server will expose a small set of static prompts for common delegation flows: review, implementation, result inspection, and stalled task recovery. Prompts are user-controlled in MCP, so they are useful for clients that surface slash-command-like workflows without turning guidance into hidden instructions.

Alternative considered: add more tools such as `usage_guide`. That would make guidance model-callable, but it pollutes the action surface with documentation and can encourage unnecessary tool calls.

### Use resources for longer guidance

The server will expose static `agent-bridge://` resources for caller workflow, safety rules, and provider capabilities. Each resource will return `text/markdown` content from an in-memory allowlist.

Alternative considered: map resources to README sections. That risks accidental filesystem reads and creates drift-prone parsing. Static in-code resources are smaller and easier to validate.

### Keep protocol version stable

The server will continue to respond with `2024-11-05` and add `prompts`/`resources` capabilities, which that revision supports. It will not add `initialize.instructions` yet because that field is documented in newer MCP revisions and this server does not currently negotiate those versions.

Alternative considered: bump to `2025-06-18` and return initialization instructions. That would improve automatic client hints, but clients that only support the current protocol could disconnect.

## Risks / Trade-offs

- Client does not surface prompts/resources automatically -> Mitigation: keep tool descriptions concise and document client-dependent behavior in README.
- Guidance drifts from README -> Mitigation: keep guidance short, workflow-oriented, and covered by protocol tests that check key phrases.
- Resource URI handling expands attack surface -> Mitigation: use a hardcoded allowlist and reject malformed or non-allowlisted URIs with JSON-RPC errors.
- Prompts are mistaken for verification -> Mitigation: prompt/resource text states that the main caller remains responsible for tests, lint, typecheck, build, and OpenSpec validation.
