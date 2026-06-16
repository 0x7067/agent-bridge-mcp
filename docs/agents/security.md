# Security

Threat model and protective mechanisms for a single-operator desktop MCP server.

## At a Glance

- No RBAC, no sessions, no JWT — security boundary is the OS user and filesystem.
- Workspace confinement prevents tasks from escaping designated directory roots.
- Children run with cleared env + provider-specific allowlists to minimize secret exposure.
- `profile: "unblocked"` is opt-in and only adds provider-specific permission-bypass flags after workspace validation succeeds.
- Prompt text is injected via PTY (Claude) or stdin, never `argv`.
- Diagnostic redaction strips keywords matching `KEY`, `TOKEN`, `SECRET` from transcripts.

## Threat Model

| Concern | Mitigation | Limitation |
|---------|------------|------------|
| Malicious provider escapes cwd | `AGENT_BRIDGE_WORKSPACES` root check + `..` rejection | Not a sandbox; abs-path traversal still possible |
| Secrets in logs | Redaction heuristic + env-clear on spawn | May miss novel secret names |
| API key theft | Keys never held by Agent Bridge | Host compromise still exposes ambient creds |
| Orphan processes | Active PID registry + panic-hook SIGTERM | Small race window around crash |
| Prompt injection via MCP | `deny_unknown_fields` + prompt length cap | Semantic injection in prompt content not defended |

## Layers

### Workspace Confinement

- Configured via `AGENT_BRIDGE_WORKSPACES` (colon-separated roots).
- Enforced in `task/spawn.rs` (`safe_cwd`): rejects paths outside roots or containing `..`.
- Unblocked provider launches still pass this check; the profile changes provider permission prompts, not Agent Bridge's workspace envelope.

### Input Sanitization

- All tool inputs: `#[serde(deny_unknown_fields)]` — hallucinated params rejected.
- Prompts capped at `MAX_PROMPT_BYTES` (100 KiB).
- Numeric arguments clamped to sane ranges.

### Secret Hygiene

- `env_clear()` on every child spawn; repopulate from provider allowlist only.
- `diagnostic_redactions()` scrubs env values containing `KEY`/`TOKEN`/`SECRET` from transcripts.
- Claude prompt injected via PTY keystrokes, never command-line arguments.

### Isolation

- `isolation: Worktree` creates disposable git worktrees on `agent-bridge/...` branches.
- Preserved until explicit `agent_remove`; auto-cleaned on crash if managed.

## Going Deeper

- [Security patterns](../SECURITY.md) — full detail on auth, authorization, data auditing, multi-tenant notes
- [Guardrails](guardrails.md) — secrets prohibition in code/commits/logs
- [Data model](data-model.md) — where transcripts, logs, and registry live on disk
