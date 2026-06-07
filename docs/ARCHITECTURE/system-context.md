# System Context Diagram (C4 Level 1)

**Last verified:** 2026-06-07

This diagram shows how `agent-bridge-mcp` fits into its environment — who uses it and what external systems it depends on.

```mermaid
C4Context
title System Context Diagram — Agent Bridge MCP

    Person(primary_agent, "Primary Coding Agent", "An AI coding agent (e.g., Claude Desktop, Kimi/Pi) that delegates bounded work to other local agents")
    Person(operator, "Human Operator", "Configures workspaces, installs provider CLIs, and monitors delegated tasks")

    System(agent_bridge, "Agent Bridge MCP", "Stdio MCP server that spawns, observes, and manages local provider agents on behalf of the primary agent")

    System_Ext(claude_cli, "Claude CLI", "Anthropic Claude Code — launched via PTY host runner")
    System_Ext(codex_cli, "Codex CLI", "OpenAI Codex CLI — noninteractive execution")
    System_Ext(cursor_cli, "Cursor Agent CLI", "Cursor IDE agent CLI — prompt mode")
    System_Ext(kimi_cli, "Kimi/Pi CLI", "Local Kimi CLI — prompt mode")
    System_ext(agv_cli, "Antigravity CLI", "AGY CLI — print/research/review mode")

    Rel(primary_agent, agent_bridge, "Delegates tasks via MCP tools", "JSON-RPC over stdio")
    Rel(operator, agent_bridge, "Configures and monitors", "Shell / File system")
    Rel(agent_bridge, claude_cli, "Spawns and controls", "Unix socket + PTY")
    Rel(agent_bridge, codex_cli, "Spawns and controls", "Process fork/exec")
    Rel(agent_bridge, cursor_cli, "Spawns and controls", "Process fork/exec")
    Rel(agent_bridge, kimi_cli, "Spawns and controls", "Process fork/exec")
    Rel(agent_bridge, agv_cli, "Spawns and controls", "Process fork/exec")
```

## Actors

| Actor | Type | Relationship to System |
|-------|------|------------------------|
| Primary Coding Agent | User | Sends MCP tool requests (e.g., `agent_spawn`, `agent_observe`) over stdio to delegate work |
| Human Operator | User | Installs provider CLIs, sets workspace/environment config, and drains/removes tasks |

## External Dependencies

| System | Purpose | Protocol | Failure Impact |
|--------|---------|----------|----------------|
| Claude CLI | Executes `research`, `review`, `implement`, `command` tasks via interactive PTY | PTY + Unix socket (host runner) | Claude provider unavailable; others unaffected |
| Codex CLI | Executes `research`, `review`, `implement` tasks noninteractively | Fork/exec + pipes | Codex provider unavailable |
| Cursor Agent CLI | Executes `research`, `review`, `implement` tasks | Fork/exec + pipes | Cursor provider unavailable |
| Kimi/Pi CLI | Executes `research`, `review`, `implement` tasks | Fork/exec + pipes | Kimi provider unavailable |
| Antigravity CLI | Executes `research`, `review` tasks (sandboxed) | Fork/exec + pipes | Antigravity provider unavailable |
| Filesystem | Stores task registry, transcripts, stdout/stderr, diffs | POSIX fs ops | Loss of persistence across restarts |
| Git | Produces diff/status for review packets | Shell invocation | Diff/status missing in `agent_result` |
