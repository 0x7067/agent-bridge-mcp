# Container Diagram (C4 Level 2)

**Last verified:** 2026-06-07

This diagram shows the major technical building blocks inside `agent-bridge-mcp` and how they communicate.

```mermaid
C4Container
title Container Diagram — Agent Bridge MCP

    Person(mcp_client, "MCP Client", "Coding agent host (Claude Desktop, VS Code, etc.) that sends JSON-RPC over stdio")

    System_Boundary(agent_bridge, "Agent Bridge MCP") {
        Container(runtime, "Runtime", "Tokio async runtime", "Manages stdio loop, panic hooks, shutdown signals, and dispatches to server")
        Container(server, "Server Router", "Rust / serde_json", "Handles JSON-RPC methods: init, tools/list, prompts/*, resources/*, and routes tool calls")
        Container(tools, "Tool Definitions", "Rust / serde", "Eight MCP tools: providers_list, doctor, agent_spawn, agent_observe, agent_result, agent_list, agent_stop, agent_remove")
        Container(task_mgr, "Task Manager", "Tokio sync primitives", "Spawns provider processes, supervises lifecycle, collects stdout/stderr/transcript, maintains registry")
        Container(provider_cmds, "Provider Commands", "Rust / std::process", "Translates provider+mode into concrete CLI invocations (claude, codex, cursor-agent, pi, agy)")
        Container(claude_host, "Claude Host Runner", "Unix-domain socket + PTY", "Sidecar subcommand that owns a PTY connected to Claude Code; bridges PTY ↔ socket")
        Container(state_store, "State Store", "Filesystem JSON", "Persists task registry, settings, and cached provider fingerprints")
    }

    System_Ext(os_shell, "Operating System", "Process management, PTYs, signals")
    System_Ext(provider_bins, "Provider Binaries", "Installed CLIs on PATH")

    Rel(mcp_client, runtime, "Sends JSON-RPC", "newline-delimited JSON over stdio")
    Rel(runtime, server, "Dispatches requests", "Function call")
    Rel(server, tools, "Resolves tool calls", "Match on ToolName")
    Rel(server, task_mgr, "Invokes lifecycle ops", "Channel/async calls")
    Rel(task_mgr, provider_cmds, "Requests command recipe", "Sync function call")
    Rel(task_mgr, os_shell, "Forks processes", "POSIX fork/exec")
    Rel(task_mgr, state_store, "Reads/Writes registry", "tokio::fs")
    Rel(claude_host, os_shell, "Opens PTY", "libc / pty-process")
    Rel(claude_host, provider_bins, "Launches claude", "PTY stdin injection")
    Rel(task_mgr, claude_host, "Connects via socket", "Unix socket + tokio::io")
```

## Containers

| Container | Technology | Purpose | Port/File |
|-----------|-----------|---------|-----------|
| Runtime | Tokio (rt-multi-thread) | Event loop, signal handling, stdio buffering | stdin/stdout descriptors |
| Server Router | Rust, serde_json | JSON-RPC dispatch, method routing, response formatting | N/A (in-process) |
| Tool Definitions | Rust, serde | Schema generation, input deserialization, annotation hints | N/A (in-process) |
| Task Manager | Tokio (sync, process, fs) | Process supervision, registry CRUD, observation streaming | `AGENT_BRIDGE_STATE_DIR` |
| Provider Commands | Rust, std::process | CLI arg construction, environment filtering, timeout defaults | N/A (in-process) |
| Claude Host Runner | pty-process, tokio net | Interactive PTY ownership for Claude; bridges to Unix socket | Unix socket path (configured) |
| State Store | tokio::fs, serde_json | Persisted registry, settings, fingerprint cache | `${STATE_DIR}/registry/*.json` |

## Communication Patterns

| From | To | Protocol | Auth | Notes |
|------|---|----------|------|-------|
| MCP Client | Runtime | ND-JSON over stdio | Implicit (subprocess trust) | Parent process relationship; server is spawned by MCP host |
| Runtime | Server | Function call | N/A | Single-threaded request handling within async runtime |
| Server | Task Manager | Async function calls | N/A | Internal module interface |
| Task Manager | OS Shell | POSIX fork/exec | PATH lookup | Restricted env whitelist per provider |
| Task Manager | Claude Host Runner | Custom framed protocol over Unix socket | FS permissions (socket owner) | Only for Claude provider; host runner must be started externally |
| Task Manager | State Store | Async file I/O | OS filesystem permissions | JSON-serialized registry records |
