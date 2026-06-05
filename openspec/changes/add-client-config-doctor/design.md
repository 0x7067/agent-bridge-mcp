## Context

Agent Bridge already has a `doctor` tool for server, workspace, state, provider, and Claude host-runner readiness. Operators also need to know whether Agent Bridge is registered in the MCP client that will call it, but Codex, Claude, and Cursor use different user-level configuration surfaces:

- Codex: `~/.codex/config.toml`
- Claude: `~/.claude.json`
- Cursor: `~/.cursor/mcp.json`

Prior setup work showed that broad home-directory searches are noisy; the diagnostic should inspect these known source-of-truth files directly.

## Goals / Non-Goals

**Goals:**

- Add read-only client configuration diagnostics to `doctor`.
- Report per-client config file presence, parse status, `agent-bridge` registration presence, command/path validation, environment-key visibility, and verification guidance.
- Keep secret values redacted and avoid task spawns or config mutation.
- Provide deterministic tests with temporary home/config fixtures.

**Non-Goals:**

- Automatically edit, install, or repair client configuration.
- Treat client registration as proof that MCP startup succeeded unless a client-specific verification command is actually run by the caller.
- Add a new dependency solely for full TOML parsing.
- Replace provider readiness or task lifecycle checks.

## Decisions

### Add diagnostics under `doctor.clients`

`doctor` is already the setup surface callers are expected to use first. Adding a `clients` section avoids another tool and keeps setup blockers in one response. The client diagnostics remain separate from provider launch readiness because they answer a different question: "is the bridge registered in the caller's client?"

Client diagnostics do not affect top-level `summary.status` in this change. Doctor cannot reliably identify the invoking MCP client today, so missing registrations in non-invoking clients would otherwise create noisy warnings. Client-specific issues appear in `doctor.clients.<client>.status` and in low-severity recommendations.

Alternative considered: add a dedicated `clients_check` tool. This would be cleaner as an API noun, but it would create another first-step diagnostic and duplicate recommendations now that `doctor` already aggregates setup health.

### Use known config paths and optional test home override

Runtime diagnostics read only the known user-level config files. Implementation should derive `~` from the process environment and support an internal/testable override path through a small helper, not through a public MCP argument.

Alternative considered: recursively search user config directories. That would produce noisy results and risks reading unrelated logs or transcripts.

### Parse enough structure for reliable diagnostics

Claude and Cursor configs are JSON and can be parsed with existing `serde_json`. Codex config is TOML, but this crate has no TOML dependency. The Codex parser should inspect the `[mcp_servers.agent-bridge]` section and recognize basic `command`, `args`, and `env` keys with conservative string/array/object handling sufficient for diagnostics.

Alternative considered: add `toml`. That would be more complete, but the current requirement is narrow and adding a dependency would require explicit user approval.

### Do not run client verification commands inside doctor

The doctor response should provide follow-up commands such as `codex mcp list` and `claude mcp list`, but it should not run them by default. Client CLIs may be slow, interactive, or host-dependent, and Cursor does not have an equivalent reliable status command in this repo context.

Alternative considered: execute verification commands when available. That would blur a cheap diagnostic with active client probing and could introduce fragile timeouts.

Shell follow-up recommendations use `kind: "shell"` and a `command` array so they are distinct from MCP tool follow-ups, which continue using `tool` and `arguments`.

### Response contract

`doctor.clients` is a keyed object:

```json
{
  "codex": {
    "client": "codex",
    "status": "ok",
    "configPath": "/Users/example/.codex/config.toml",
    "configPresent": true,
    "parseStatus": "ok",
    "registrationStatus": "registered",
    "command": {
      "value": "/Users/example/.local/bin/agent-bridge-mcp",
      "status": "ok",
      "resolution": "absolute_exists"
    },
    "args": [],
    "envKeys": ["AGENT_BRIDGE_WORKSPACES"],
    "verificationStatus": "not_verified",
    "verificationCommands": [
      {
        "kind": "shell",
        "command": ["codex", "mcp", "list"],
        "description": "Verify Codex can load the registered MCP server."
      }
    ],
    "recommendations": []
  }
}
```

Per-client `status` values are `ok`, `info`, `warning`, or `error`. `error` is reserved for unreadable or unparseable present config files. Missing config and absent registrations are `info` because they may be irrelevant for users who do not use that client.

## Risks / Trade-offs

- Codex targeted TOML parsing may miss unusual formatting -> Report parse confidence and keep diagnostics conservative when the section cannot be interpreted.
- Registered config may still fail at runtime -> Return `verificationStatus: "not_verified"` and structured follow-up commands instead of claiming startup success.
- Config files may contain secrets -> Report env key names and redacted indicators only; never echo raw values.
- Cursor verification remains weaker -> Clearly mark Cursor as file-inspected only unless a future reliable CLI status surface is added.
- User-level-only inspection can miss project-level MCP overrides -> Document that this diagnostic intentionally reads only the known global client config files.
