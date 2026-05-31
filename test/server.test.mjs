import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { buildCommand, buildProviderEnv, handleRequest, runCommand, validateToolArguments } from "../src/server.mjs";

test("initialize returns MCP server capabilities", async () => {
  const response = await handleRequest({ jsonrpc: "2.0", id: 1, method: "initialize", params: {} });
  assert.equal(response.result.protocolVersion, "2024-11-05");
  assert.deepEqual(response.result.capabilities, { tools: {} });
});

test("tools/list exposes agent bridge tools", async () => {
  const response = await handleRequest({ jsonrpc: "2.0", id: 2, method: "tools/list", params: {} });
  const names = response.result.tools.map((tool) => tool.name);
  assert.equal(names.length, 5);
  for (const name of ["ask_claude", "ask_kimi", "ask_cursor", "dispatch_claude", "dispatch_cursor"]) {
    assert.ok(names.includes(name), `${name} should be listed`);
  }
});

test("builds Claude command through claude-p", async () => {
  const command = await buildCommand("ask_claude", {
    prompt: "Review this",
    cwd: process.cwd(),
    timeoutSeconds: 10,
    dryRun: true
  });
  assert.equal(command.command, "claude-p");
  assert.deepEqual(command.args.slice(0, 6), ["--cwd", process.cwd(), "--timeout", "10", "--output-format", "json"]);
  assert.ok(command.args.includes("--"));
  assert.ok(command.args.includes("Review this"));
});

test("builds Kimi command through existing wrapper", async () => {
  const command = await buildCommand("ask_kimi", { prompt: "Review this", dryRun: true });
  assert.equal(command.command, path.join(os.homedir(), ".claude/skills/kimi-review/kimi.sh"));
  assert.deepEqual(command.args.slice(0, 2), ["consult", "Review this"]);
});

test("builds Cursor read-only ask command", async () => {
  const command = await buildCommand("ask_cursor", { prompt: "Review this", cwd: process.cwd(), dryRun: true });
  assert.equal(command.command, "cursor-agent");
  assert.ok(command.args.includes("--mode"));
  assert.ok(command.args.includes("ask"));
  assert.ok(command.args.includes("--"));
  assert.ok(command.args.includes("Review this"));
});

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

test("rejects oversized prompts by UTF-8 byte length", async () => {
  const prompt = "é".repeat(51201);
  assert.ok(Buffer.byteLength(prompt, "utf8") > 102400);
  const response = await handleRequest({
    jsonrpc: "2.0",
    id: 8,
    method: "tools/call",
    params: { name: "ask_kimi", arguments: { prompt, dryRun: true } }
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

test("allowed root rejects omitted cwd when default cwd is outside root", async () => {
  await assert.rejects(
    validateToolArguments("ask_claude", { prompt: "Review this" }, { allowedRoot: os.tmpdir(), defaultCwd: process.cwd() }),
    /outside allowed root/
  );
});

test("allowed root rejects symlink cwd escapes", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "agent-bridge-root-"));
  const outside = await fs.mkdtemp(path.join(os.tmpdir(), "agent-bridge-outside-"));
  const link = path.join(root, "escape");
  await fs.symlink(outside, link);
  await assert.rejects(
    validateToolArguments("ask_claude", { prompt: "Review this", cwd: link }, { allowedRoot: root, defaultCwd: root }),
    /outside allowed root/
  );
});

test("allowed root rejects symlink context file escapes", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "agent-bridge-root-"));
  const outside = await fs.mkdtemp(path.join(os.tmpdir(), "agent-bridge-outside-"));
  await fs.writeFile(path.join(outside, "secret.txt"), "secret");
  await fs.symlink(path.join(outside, "secret.txt"), path.join(root, "secret-link.txt"));
  await assert.rejects(
    validateToolArguments("ask_kimi", { prompt: "Review this", cwd: root, contextFiles: ["secret-link.txt"] }, { allowedRoot: root, defaultCwd: root }),
    /outside cwd/
  );
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

test("runner truncates oversized stdout and stderr", async () => {
  const result = await runCommand(process.execPath, ["-e", "process.stdout.write('x'.repeat(1048600)); process.stderr.write('y'.repeat(1048600))"], {
    timeoutMs: 1000,
    cwd: process.cwd(),
    maxBufferBytes: 1048576
  });
  assert.equal(result.ok, true);
  assert.ok(Buffer.byteLength(result.stdout, "utf8") <= 1048576);
  assert.ok(Buffer.byteLength(result.stderr, "utf8") <= 1048576);
  assert.match(result.stdout, /\[truncated after 1048576 bytes\]$/);
  assert.match(result.stderr, /\[truncated after 1048576 bytes\]$/);
});

test("dispatch builders allow safe capability options", async () => {
  const command = await buildCommand("dispatch_claude", {
    prompt: "Implement this",
    cwd: process.cwd(),
    permissionMode: "default",
    allowedTools: ["Read", "Grep", "Edit"],
    dryRun: true
  });
  assert.ok(command.args.includes("Edit"));
});

test("dispatch builders reject unsafe permission modes", async () => {
  await assert.rejects(
    buildCommand("dispatch_claude", {
      prompt: "Implement this",
      cwd: process.cwd(),
      permissionMode: "bypassPermissions",
      dryRun: true
    }),
    /permissionMode/
  );
});

test("dispatch cursor builder includes model and boolean trust flag", async () => {
  const command = await buildCommand("dispatch_cursor", {
    prompt: "Implement this",
    cwd: process.cwd(),
    model: "composer-2.5",
    permissionMode: "default",
    dryRun: true
  });
  assert.equal(command.command, "cursor-agent");
  assert.ok(command.args.includes("--model"));
  assert.ok(command.args.includes("composer-2.5"));
  assert.ok(command.args.includes("--trust"));
  assert.ok(command.args.includes("--"));
});

test("timeout seconds are clamped in provider commands", async () => {
  const low = await buildCommand("ask_claude", { prompt: "Review this", cwd: process.cwd(), timeoutSeconds: 0, dryRun: true });
  const high = await buildCommand("ask_claude", { prompt: "Review this", cwd: process.cwd(), timeoutSeconds: 9999, dryRun: true });
  assert.equal(low.args[low.args.indexOf("--timeout") + 1], "1");
  assert.equal(high.args[high.args.indexOf("--timeout") + 1], "1800");
});

test("provider env preserves terminal variables needed by claude-p", () => {
  const previousTerm = process.env.TERM;
  process.env.TERM = "xterm-256color";
  try {
    const env = buildProviderEnv();
    assert.equal(env.TERM, "xterm-256color");
  } finally {
    if (previousTerm === undefined) {
      delete process.env.TERM;
    } else {
      process.env.TERM = previousTerm;
    }
  }
});
