# Agent Bridge MCP

Dependency-free stdio MCP server for asking local agent CLIs for second opinions or bounded delegated work from Codex.

## Tools

- `ask_claude`: read-only Claude Code second opinion through `claude-p`.
- `ask_kimi`: read-only Kimi/Pi consult through `kimi.sh`; supports `contextFiles`.
- `ask_cursor`: read-only Cursor Agent second opinion through `cursor-agent --mode ask`.
- `dispatch_claude`: bounded Claude Code dispatch with explicit safe capability options.
- `dispatch_cursor`: bounded Cursor Agent dispatch with optional model selection.

## Requirements

- Node.js 24 or newer.
- `claude-p` on `PATH`, or set `CLAUDE_P_BIN`.
- Kimi wrapper at `~/.claude/skills/kimi-review/kimi.sh`, or set `KIMI_WRAPPER_PATH`.
- `cursor-agent` on `PATH`, or set `CURSOR_AGENT_BIN`.

`ask_kimi` covers Pi/Kimi through the existing hardened wrapper. This server intentionally does not expose raw write-capable Pi.

## Safety

- Public tool arguments are whitelisted; unknown fields are rejected.
- Read-only tools reject capability overrides.
- Dispatch tools accept only `permissionMode: "dontAsk"` or `"default"`.
- `cwd` and `contextFiles` are validated with `fs.realpath` to block symlink escapes.
- Set `AGENT_BRIDGE_ALLOWED_ROOT` to confine calls to a workspace root.
- Prompts are capped at 100 KiB UTF-8.
- Provider stdout/stderr are capped at 1 MiB each.
- Provider processes use ignored stdin, timeouts, and the current local session environment. This is required for Claude/Cursor auth and PTY startup; run this MCP server only with trusted local agent CLIs.

Live provider calls can spend tokens. Use `dryRun: true` to inspect commands without launching providers.

## Codex MCP Config

Use an absolute path to `src/server.mjs`:

```json
{
  "mcpServers": {
    "agent-bridge": {
      "command": "node",
      "args": [
        "/Users/pedro/Documents/Codex/2026-05-31/figure-out-a-way-that-you/src/server.mjs"
      ],
      "env": {
        "AGENT_BRIDGE_ALLOWED_ROOT": "/Users/pedro/Documents/Codex/2026-05-31/figure-out-a-way-that-you"
      }
    }
  }
}
```

## Examples

Dry-run a Claude call:

```json
{
  "name": "ask_claude",
  "arguments": {
    "prompt": "Review this plan for correctness risks.",
    "dryRun": true
  }
}
```

Ask Kimi with local context:

```json
{
  "name": "ask_kimi",
  "arguments": {
    "prompt": "Review this MCP server implementation.",
    "cwd": "/Users/pedro/Documents/Codex/2026-05-31/figure-out-a-way-that-you",
    "contextFiles": ["src/server.mjs", "test/server.test.mjs"]
  }
}
```

Run tests:

```bash
npm test
```
