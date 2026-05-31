# Agent Bridge MCP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a stdio MCP server that lets Codex ask Claude Code, Pi/Kimi, and Cursor for bounded second opinions or delegated work through local CLI wrappers.

**Architecture:** Implement a dependency-free Node.js JSON-RPC MCP server with a small tool registry, a command runner with timeout, cwd controls, isolated child-process stdin, and provider wrappers for `claude-p`, `kimi.sh`, and `cursor-agent`. Default tools are second-opinion/read-only; dispatch tools are explicit and still bounded by timeout, cwd, byte caps, and provider flags.

**Tech Stack:** Node.js 24 stdlib, MCP JSON-RPC over stdio, `node:test`, local CLIs (`claude-p`, `~/.claude/skills/kimi-review/kimi.sh`, `cursor-agent`).

---

### File Structure

- Create `package.json`: scripts for `test`, `start`, and package metadata.
- Create `src/server.mjs`: stdio JSON-RPC loop, MCP methods, tool schemas, provider command construction, process execution.
- Create `test/server.test.mjs`: protocol-level tests for initialization, tool listing, dry-run command construction, missing-tool errors, and timeout-safe runner behavior.
- Create `README.md`: install/register instructions, tool list, safety posture, and examples.
- Create `codex-mcp.example.json`: copyable MCP server config snippet for Codex.

### Safety Rules

- Public MCP tool arguments are whitelisted. Unknown fields are rejected.
- `cwd` is optional. If omitted, use the server process cwd.
- `cwd` must resolve to an absolute existing real directory under `AGENT_BRIDGE_ALLOWED_ROOT` when that environment variable is set; otherwise it must resolve under the real server startup cwd. Reject paths with literal `..` segments before resolution. Use `fs.realpath` before prefix checks so symlinks cannot escape the allowed root.
- `contextFiles` must be relative paths without `..` segments and must resolve via `fs.realpath` to regular files inside the resolved real `cwd`.
- `prompt` is required and capped at 100 KiB UTF-8.
- Read-only tools reject `permissionMode`, `allowedTools`, and `disallowedTools`.
- Dispatch tools allow `permissionMode` only when it is `dontAsk` or `default`.
- No public tool accepts raw command, raw argv, shell snippets, or provider command overrides.
- Provider processes receive an explicit environment allowlist: `PATH`, `HOME`, `TMPDIR`, `CLAUDE_P_BIN`, `KIMI_WRAPPER_PATH`, `KIMI_MODEL`, `KIMI_THINKING`, `CURSOR_AGENT_BIN`, `CURSOR_API_KEY`, and `AGENT_BRIDGE_ALLOWED_ROOT` when present.
- `contextFiles` are supported only by `ask_kimi`; Claude and Cursor tools reject them.
- Provider invocations insert `--` before prompt positional arguments where the target CLI supports it (`claude-p`, `cursor-agent`) so prompts beginning with `--` cannot become flags.
- The resolved default cwd is validated against `AGENT_BRIDGE_ALLOWED_ROOT`. If the server cwd is outside the allowed root and a tool call omits `cwd`, reject the call.

### Task 1: MCP Protocol Surface

**Files:**
- Create: `package.json`
- Create: `src/server.mjs`
- Test: `test/server.test.mjs`

- [ ] **Step 1: Write failing protocol tests**

```js
import test from "node:test";
import assert from "node:assert/strict";
import { handleRequest } from "../src/server.mjs";

test("initialize returns MCP server capabilities", async () => {
  const response = await handleRequest({ jsonrpc: "2.0", id: 1, method: "initialize", params: {} });
  assert.equal(response.result.protocolVersion, "2024-11-05");
  assert.deepEqual(response.result.capabilities, { tools: {} });
});

test("tools/list exposes agent bridge tools", async () => {
  const response = await handleRequest({ jsonrpc: "2.0", id: 2, method: "tools/list", params: {} });
  const names = response.result.tools.map((tool) => tool.name);
  assert.deepEqual(names, ["ask_claude", "ask_kimi", "ask_cursor", "dispatch_claude", "dispatch_cursor"]);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm test`
Expected: FAIL with `Cannot find module '../src/server.mjs'`.

- [ ] **Step 3: Implement minimal MCP handlers and stdio transport**

Implement `handleRequest()` with `initialize`, `notifications/initialized`, `tools/list`, `tools/call`, and JSON-RPC errors. Export helpers for tests. Implement a newline-delimited JSON-RPC stdio loop that buffers `process.stdin`, parses one JSON request per line, ignores notifications without ids, writes JSON responses plus `\n` to `process.stdout`, and exits cleanly on stdin EOF.

- [ ] **Step 4: Run test to verify it passes**

Run: `npm test`
Expected: protocol tests PASS.

### Task 2: Provider Command Construction

**Files:**
- Modify: `src/server.mjs`
- Modify: `test/server.test.mjs`

- [ ] **Step 1: Write failing command-construction tests**

```js
import os from "node:os";
import path from "node:path";
import { buildCommand } from "../src/server.mjs";

test("builds Claude command through claude-p", () => {
  const command = buildCommand("ask_claude", { prompt: "Review this", cwd: "/tmp/project", timeoutSeconds: 10, dryRun: true });
  assert.equal(command.command, "claude-p");
  assert.deepEqual(command.args.slice(0, 6), ["--cwd", "/tmp/project", "--timeout", "10", "--output-format", "json"]);
  assert.ok(command.args.includes("--"));
  assert.ok(command.args.includes("Review this"));
});

test("builds Kimi command through existing wrapper", () => {
  const command = buildCommand("ask_kimi", { prompt: "Review this", dryRun: true });
  assert.equal(command.command, path.join(os.homedir(), ".claude/skills/kimi-review/kimi.sh"));
  assert.deepEqual(command.args.slice(0, 2), ["consult", "Review this"]);
});

test("builds Cursor read-only ask command", () => {
  const command = buildCommand("ask_cursor", { prompt: "Review this", cwd: "/tmp/project", dryRun: true });
  assert.equal(command.command, "cursor-agent");
  assert.ok(command.args.includes("--mode"));
  assert.ok(command.args.includes("ask"));
  assert.ok(command.args.includes("--"));
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm test`
Expected: FAIL with `buildCommand` missing or returning incomplete commands.

- [ ] **Step 3: Implement provider builders**

Add builders for:
- `ask_claude`: `(process.env.CLAUDE_P_BIN || "claude-p") --cwd <cwd> --timeout <seconds> --output-format json --permission-mode dontAsk --allowedTools Read,Grep,Glob --disallowedTools Bash,Edit,Write <prompt>`
- `dispatch_claude`: same `claude-p` path, but allow caller-provided `permissionMode`, `allowedTools`, and `disallowedTools`.
- `ask_kimi`: `(process.env.KIMI_WRAPPER_PATH || path.join(os.homedir(), ".claude/skills/kimi-review/kimi.sh")) consult <prompt> [contextFiles...]`
- `ask_cursor`: `(process.env.CURSOR_AGENT_BIN || "cursor-agent") -p --mode ask --output-format json --workspace <cwd> --trust -- <prompt>`
- `dispatch_cursor`: `cursor-agent -p --output-format json --workspace <cwd> --trust -- <prompt>`, with optional `model`.

For read-only tools (`ask_claude`, `ask_kimi`, `ask_cursor`), reject caller-supplied `permissionMode`, `allowedTools`, and `disallowedTools`. Only dispatch tools may accept capability-related options.

- [ ] **Step 4: Run test to verify it passes**

Run: `npm test`
Expected: command-construction tests PASS.

### Task 3: Tool Calls and Runner Behavior

**Files:**
- Modify: `src/server.mjs`
- Modify: `test/server.test.mjs`

- [ ] **Step 1: Write failing dry-run and validation tests**

```js
test("tools/call supports dry-run without spawning provider", async () => {
  const response = await handleRequest({
    jsonrpc: "2.0",
    id: 3,
    method: "tools/call",
    params: { name: "ask_claude", arguments: { prompt: "Review this", dryRun: true } }
  });
  assert.equal(response.result.isError, false);
  assert.match(response.result.content[0].text, /claude-p/);
});

test("tools/call rejects missing prompt", async () => {
  const response = await handleRequest({
    jsonrpc: "2.0",
    id: 4,
    method: "tools/call",
    params: { name: "ask_cursor", arguments: { dryRun: true } }
  });
  assert.equal(response.result.isError, true);
  assert.match(response.result.content[0].text, /prompt is required/);
});

test("read-only tools reject capability overrides", async () => {
  const response = await handleRequest({
    jsonrpc: "2.0",
    id: 5,
    method: "tools/call",
    params: {
      name: "ask_claude",
      arguments: { prompt: "Review this", allowedTools: ["Edit"], dryRun: true }
    }
  });
  assert.equal(response.result.isError, true);
  assert.match(response.result.content[0].text, /not allowed for read-only tools/);
});

test("context files must stay under cwd", async () => {
  const response = await handleRequest({
    jsonrpc: "2.0",
    id: 6,
    method: "tools/call",
    params: {
      name: "ask_kimi",
      arguments: { prompt: "Review this", contextFiles: ["../secret.txt"], dryRun: true }
    }
  });
  assert.equal(response.result.isError, true);
  assert.match(response.result.content[0].text, /contextFiles/);
});

test("Claude and Cursor reject context files", async () => {
  const response = await handleRequest({
    jsonrpc: "2.0",
    id: 7,
    method: "tools/call",
    params: {
      name: "ask_cursor",
      arguments: { prompt: "Review this", contextFiles: ["README.md"], dryRun: true }
    }
  });
  assert.equal(response.result.isError, true);
  assert.match(response.result.content[0].text, /contextFiles are only supported by ask_kimi/);
});

test("rejects oversized prompts", async () => {
  const response = await handleRequest({
    jsonrpc: "2.0",
    id: 8,
    method: "tools/call",
    params: {
      name: "ask_kimi",
      arguments: { prompt: "x".repeat(102401), dryRun: true }
    }
  });
  assert.equal(response.result.isError, true);
  assert.match(response.result.content[0].text, /prompt exceeds/);
});

test("rejects cwd containing literal dot-dot segments", async () => {
  const response = await handleRequest({
    jsonrpc: "2.0",
    id: 9,
    method: "tools/call",
    params: {
      name: "ask_claude",
      arguments: { prompt: "Review this", cwd: "/tmp/foo/../bar", dryRun: true }
    }
  });
  assert.equal(response.result.isError, true);
  assert.match(response.result.content[0].text, /cwd must not contain/);
});

test("runner reports timeout errors", async () => {
  const result = await runCommand(process.execPath, ["-e", "setTimeout(() => {}, 1000)"], {
    timeoutMs: 100,
    cwd: process.cwd()
  });
  assert.equal(result.ok, false);
  assert.match(result.error, /timed out/i);
});

test("runner reports provider failures", async () => {
  const result = await runCommand(process.execPath, ["-e", "console.error('provider failed'); process.exit(7)"], {
    timeoutMs: 1000,
    cwd: process.cwd()
  });
  assert.equal(result.ok, false);
  assert.equal(result.exitCode, 7);
  assert.match(result.stderr, /provider failed/);
});

test("runner truncates oversized stdout", async () => {
  const result = await runCommand(process.execPath, ["-e", "process.stdout.write('x'.repeat(1048600))"], {
    timeoutMs: 1000,
    cwd: process.cwd(),
    maxBufferBytes: 1048576
  });
  assert.equal(result.ok, true);
  assert.ok(Buffer.byteLength(result.stdout, "utf8") <= 1048576);
  assert.match(result.stdout, /\[truncated after 1048576 bytes\]$/);
});

test("dispatch builders allow explicit capability options", () => {
  const command = buildCommand("dispatch_claude", {
    prompt: "Implement this",
    cwd: "/tmp/project",
    permissionMode: "default",
    allowedTools: ["Read", "Grep", "Edit"],
    dryRun: true
  });
  assert.ok(command.args.includes("Edit"));
});

test("dispatch builders reject unsafe permission modes", () => {
  assert.throws(() => buildCommand("dispatch_claude", {
    prompt: "Implement this",
    cwd: "/tmp/project",
    permissionMode: "bypassPermissions",
    dryRun: true
  }), /permissionMode/);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm test`
Expected: FAIL because `tools/call` has no dry-run execution path or validation.

- [ ] **Step 3: Implement validation and runner**

Add prompt/cwd/context file validation per the Safety Rules, dry-run output, process spawning with `AbortController`, bounded stdout/stderr capture, and MCP content results. Keep public `timeoutSeconds` clamped to 1-1800 seconds and test both low and high clamp cases through command construction. Apply the same `permissionMode` whitelist to `dispatch_claude` and `dispatch_cursor`. Unit-test the runner through an exported `runCommand()` helper that accepts `timeoutMs`, but never expose command overrides or raw argv controls through MCP tool arguments. Always spawn child processes with `stdio: ["ignore", "pipe", "pipe"]` and an explicit environment allowlist so providers cannot consume MCP stdin or inherit unrelated secrets. Cap stdout and stderr at 1 MiB each; when truncating, reserve bytes for `[truncated after 1048576 bytes]` and replace the tail so the final string is no larger than the cap. Track active child processes and on `SIGINT` or `SIGTERM`, call `child.kill("SIGTERM")` for each tracked child before letting the server exit. Write one-line provider start/finish/failure logs to stderr without including prompt text.

- [ ] **Step 4: Run test to verify it passes**

Run: `npm test`
Expected: dry-run and validation tests PASS.

### Task 4: Documentation and Registration

**Files:**
- Create: `README.md`
- Create: `codex-mcp.example.json`

- [ ] **Step 1: Write docs**

Document tools, examples, safety defaults, requirements, and registration:

```json
{
  "mcpServers": {
    "agent-bridge": {
      "command": "node",
      "args": ["/absolute/path/to/src/server.mjs"]
    }
  }
}
```

Clarify that Pi is covered through the existing Kimi wrapper by default, not through a write-capable raw Pi tool. Document provider path environment variables: `CLAUDE_P_BIN`, `KIMI_WRAPPER_PATH`, and `CURSOR_AGENT_BIN`.
Include exact package metadata:

```json
{
  "type": "module",
  "scripts": {
    "start": "node src/server.mjs",
    "test": "node --test"
  }
}
```

Document that MCP config `args` must use an absolute path to `src/server.mjs`; from this workspace it is `/Users/pedro/Documents/Codex/2026-05-31/figure-out-a-way-that-you/src/server.mjs`.

- [ ] **Step 2: Run structural checks**

Run: `npm test`
Expected: all tests PASS.

Run: `node src/server.mjs` with one-line initialize/tools/list JSON-RPC requests.
Expected: valid JSON-RPC responses on stdout.

### Task 5: Final Verification

**Files:**
- All created files

- [ ] **Step 1: Run full tests**

Run: `npm test`
Expected: all tests PASS.

- [ ] **Step 2: Run MCP smoke test**

Run: a direct stdio JSON-RPC smoke test for `initialize`, `tools/list`, and `tools/call` with `{ "dryRun": true }`.
Expected: responses include protocol version, all five tool names, and a dry-run command string.

- [ ] **Step 3: Report exact verification output**

Summarize command outputs and any limitations, including that live Claude/Pi/Cursor model calls were not run unless explicitly requested.
