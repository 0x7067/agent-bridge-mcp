# Container Diagram (C4 Level 2)

**Last verified:** 2026-06-07

This diagram shows the major technical building blocks inside `agent-bridge-mcp` and how they communicate.

```mermaid
C4Container
title Container Diagram — Agent Bridge MCP

    Person(agent_client, "Agent Client", "Coding agent host that sends ACP JSON-RPC or uses the MCP adapter over stdio")

    System_Boundary(agent_bridge, "Agent Bridge MCP") {
        Container(runtime, "Runtime", "Tokio async runtime", "Manages CLI routing, stdio loops, panic hooks, and shutdown signals")
        Container(router, "ACP Router", "Rust / serde_json", "Handles initialize, session/new, and session/prompt by routing one provider turn")
        Container(adapter, "MCP Adapter", "Rust / serde", "Two MCP tools: agent_delegate and agent_evidence")
        Container(task_mgr, "Task Manager", "Tokio sync primitives", "Spawns provider processes, supervises lifecycle, collects stdout/stderr/transcript, maintains registry")
        Container(provider_cmds, "Provider Commands", "Rust / std::process", "Translates provider+mode into concrete CLI invocations (claude, codex, cursor-agent, pi, forge, agy)")
        Container(claude_host, "Claude Host Runner", "Unix-domain socket + PTY", "Sidecar subcommand that owns a PTY connected to Claude Code; bridges PTY ↔ socket")
        Container(state_store, "State Store", "Filesystem JSON", "Persists task registry, settings, and cached provider fingerprints")
    }

    System_Ext(os_shell, "Operating System", "Process management, PTYs, signals")
    System_Ext(provider_bins, "Provider Binaries", "Installed CLIs on PATH")

    Rel(agent_client, runtime, "Sends JSON-RPC", "newline-delimited JSON over stdio")
    Rel(runtime, router, "Dispatches default ACP requests", "Function call")
    Rel(runtime, adapter, "Dispatches mcp-adapter requests", "Function call")
    Rel(router, task_mgr, "Runs routed turns", "Channel/async calls")
    Rel(adapter, task_mgr, "Delegates turns / reads evidence", "Channel/async calls")
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
| ACP Router | Rust, serde_json | Default JSON-RPC dispatch for `initialize`, `session/new`, `session/prompt` | N/A (in-process) |
| MCP Adapter | Rust, serde | Two-tool adapter for `agent_delegate` and `agent_evidence` | N/A (in-process) |
| Task Manager | Tokio (sync, process, fs) | Process supervision, registry CRUD, observation streaming | `AGENT_BRIDGE_STATE_DIR` |
| Provider Commands | Rust, std::process | CLI arg construction, environment filtering, timeout defaults | N/A (in-process) |
| Claude Host Runner | pty-process, tokio net | Interactive PTY ownership for Claude; bridges to Unix socket | Unix socket path (configured) |
| State Store | tokio::fs, serde_json | Persisted registry, settings, fingerprint cache | `${STATE_DIR}/registry/*.json` |

## Communication Patterns

| From | To | Protocol | Auth | Notes |
|------|---|----------|------|-------|
| Agent Client | Runtime | ND-JSON over stdio | Implicit (subprocess trust) | Parent process relationship; runtime is spawned by the host |
| Runtime | ACP Router / MCP Adapter | Function call | N/A | Single-threaded request handling within async runtime |
| Router / Adapter | Task Manager | Async function calls | N/A | Internal module interface |
| Task Manager | OS Shell | POSIX fork/exec | PATH lookup | Restricted env whitelist per provider |
| Task Manager | Claude Host Runner | Custom framed protocol over Unix socket | FS permissions (socket owner) | Only for Claude provider; host runner must be started externally |
| Task Manager | State Store | Async file I/O | OS filesystem permissions | JSON-serialized registry records |
