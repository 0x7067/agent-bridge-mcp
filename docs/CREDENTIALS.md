# Credentials and Access Inventory

**Last verified:** 2026-06-07

IMPORTANT: This document describes where credentials are stored, not what they are. Never commit actual secrets to version control.

## Credential Storage

This project does not define dedicated secret environment variables. Instead, it relies on ambient host credentials and filesystem isolation.

| Credential | Source | Provider | Storage Location | Rotation |
|------------|--------|----------|-----------------|----------|
| Provider API keys | Ambient host | Claude / Codex / Cursor / Kimi / Antigravity | Host keychain, cloud creds, or provider config dirs | Per provider schedule |
| Git identity | Ambient host | Git operations | `~/.gitconfig` or env vars | As needed |
| Unix socket ACL | Filesystem | Claude host runner | Socket file permissions | On socket recreation |

### External Services

No outbound API keys are owned by `agent-bridge-mcp` itself. The server acts as a launcher and supervisor for provider CLIs that may internally authenticate to their respective backends.

| Credential | Env Var | Storage | Used By |
|------------|---------|---------|---------|
| Not applicable | — | — | — |

### Authentication

No JWT, session, or bespoke auth scheme is implemented. The security boundary is the host machine and the MCP client's subprocess relationship.

| Credential | Mechanism | Boundary |
|------------|-----------|----------|
| Subprocess trust | Inherited UID/GID | MCP host + launched server share OS identity |

## Access Requirements for New Team Members

To contribute to this project, a new team member needs:

1. Rust toolchain — install via [rustup.rs](https://rustup.rs/)
2. `git` configured with signing/email (follow project commit conventions)
3. Optional: provider CLI installations for local smoke testing (e.g., `claude`, `codex`, `cursor-agent`, `pi`, `agy`)
4. No cloud accounts, VPNs, or SaaS dashboards are required for development

## Local Development Credentials

Copy and customize environment variables as needed:

```bash
export AGENT_BRIDGE_WORKSPACES="$HOME/projects:$HOME/oss"
export AGENT_BRIDGE_STATE_DIR="$HOME/.agent-bridge-mcp/state"
```

If testing the Claude provider locally, also start the host runner and export its socket path:

```bash
export AGENT_BRIDGE_CLAUDE_HOST_SOCKET="$HOME/.agent-bridge-mcp/run/claude-host.sock"
```

Provider CLIs themselves may prompt for login or read credentials from their own config files (outside the scope of this project).
