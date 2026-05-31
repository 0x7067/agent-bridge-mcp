import test from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { Readable } from "node:stream";
import {
  buildProviderEnv,
  buildTaskCommand,
  createTaskManager,
  handleRequest,
  loadRegistry,
  readCappedFile,
  validateTaskSpawnArguments
} from "../src/server.mjs";

async function tempDir(prefix = "agent-bridge-test-") {
  return fs.mkdtemp(path.join(os.tmpdir(), prefix));
}

function fakeChild({ stdout = "", stderr = "", exitCode = 0, signal = null, delayMs = 5 } = {}) {
  const child = new EventEmitter();
  child.pid = Math.floor(Math.random() * 100000) + 1000;
  child.stdout = Readable.from([stdout]);
  child.stderr = Readable.from([stderr]);
  child.killCalls = [];
  child.kill = (killSignal = "SIGTERM") => {
    child.killCalls.push(killSignal);
    setTimeout(() => child.emit("close", null, killSignal), 0);
    return true;
  };
  setTimeout(() => child.emit("close", exitCode, signal), delayMs);
  return child;
}

async function waitForStatus(manager, taskId, expectedStatus) {
  const deadline = Date.now() + 1000;
  let status;
  do {
    status = await manager.status(taskId);
    if (status.status === expectedStatus) {
      return status;
    }
    await new Promise((resolve) => setTimeout(resolve, 10));
  } while (Date.now() < deadline);
  assert.equal(status.status, expectedStatus);
}

async function waitForRegistryStatus(stateDir, taskId, expectedStatus) {
  const deadline = Date.now() + 1000;
  let registry;
  do {
    registry = await loadRegistry(stateDir);
    if (registry.tasks[taskId]?.status === expectedStatus) {
      return registry;
    }
    await new Promise((resolve) => setTimeout(resolve, 10));
  } while (Date.now() < deadline);
  assert.equal(registry.tasks[taskId]?.status, expectedStatus);
}

test("initialize returns MCP server capabilities", async () => {
  const response = await handleRequest({ jsonrpc: "2.0", id: 1, method: "initialize", params: {} });
  assert.equal(response.result.protocolVersion, "2024-11-05");
  assert.deepEqual(response.result.capabilities, { tools: {} });
});

test("tools/list exposes only task-native tools", async () => {
  const response = await handleRequest({ jsonrpc: "2.0", id: 2, method: "tools/list", params: {} });
  const names = response.result.tools.map((tool) => tool.name);
  assert.deepEqual(names, [
    "providers_list",
    "task_spawn",
    "task_list",
    "task_status",
    "task_logs",
    "task_result",
    "task_stop",
    "task_remove"
  ]);
});

test("providers_list reports first-class provider capabilities", async () => {
  const response = await handleRequest({
    jsonrpc: "2.0",
    id: 3,
    method: "tools/call",
    params: { name: "providers_list", arguments: {} }
  });
  assert.equal(response.result.isError, false);
  const payload = JSON.parse(response.result.content[0].text);
  assert.deepEqual(Object.keys(payload.providers), ["claude", "cursor", "kimi", "codex"]);
  assert.equal(payload.providers.claude.supportsReply, false);
  assert.ok(payload.providers.codex.modes.includes("implement"));
});

test("validates task_spawn arguments and rejects legacy or unsupported inputs", async () => {
  await assert.rejects(validateTaskSpawnArguments({ provider: "openai", mode: "review", prompt: "x" }), /provider/);
  await assert.rejects(validateTaskSpawnArguments({ provider: "claude", mode: "chat", prompt: "x" }), /mode/);
  await assert.rejects(validateTaskSpawnArguments({ provider: "cursor", mode: "command", prompt: "x" }), /does not support mode/);
  await assert.rejects(validateTaskSpawnArguments({ provider: "kimi", mode: "implement" }), /prompt is required/);
  await assert.rejects(validateTaskSpawnArguments({ provider: "codex", mode: "review", prompt: "x", maxTurns: 2 }), /Unknown argument/);
});

test("tools/call rejects unknown arguments on lifecycle tools", async () => {
  const response = await handleRequest({
    jsonrpc: "2.0",
    id: 4,
    method: "tools/call",
    params: { name: "task_status", arguments: { taskId: "task_x", prompt: "legacy" } }
  });
  assert.equal(response.result.isError, true);
  assert.match(response.result.content[0].text, /Unknown argument/);
});

test("validates cwd with allowed root and rejects symlink escapes", async () => {
  const root = await tempDir("agent-bridge-root-");
  const outside = await tempDir("agent-bridge-outside-");
  const link = path.join(root, "escape");
  await fs.symlink(outside, link);

  await assert.rejects(
    validateTaskSpawnArguments(
      { provider: "codex", mode: "review", prompt: "x", cwd: link },
      { allowedRoot: root, defaultCwd: root }
    ),
    /outside allowed root/
  );
});

test("builds provider commands for explicit modes", async () => {
  const cwd = process.cwd();
  const previousClaude = process.env.CLAUDE_BIN;
  const previousClaudeP = process.env.CLAUDE_P_BIN;
  delete process.env.CLAUDE_BIN;
  delete process.env.CLAUDE_P_BIN;
  const claude = await buildTaskCommand({ provider: "claude", mode: "review", prompt: "Review", cwd });
  if (previousClaude === undefined) {
    delete process.env.CLAUDE_BIN;
  } else {
    process.env.CLAUDE_BIN = previousClaude;
  }
  if (previousClaudeP === undefined) {
    delete process.env.CLAUDE_P_BIN;
  } else {
    process.env.CLAUDE_P_BIN = previousClaudeP;
  }

  assert.equal(claude.command, "claude-p");
  assert.deepEqual(claude.args.slice(0, 8), ["--cwd", cwd, "--timeout", "120", "--output-format", "json", "--permission-mode", "dontAsk"]);
  assert.ok(claude.args.includes("--allowedTools"));

  const cursor = await buildTaskCommand({ provider: "cursor", mode: "implement", prompt: "Implement", cwd, model: "gpt-5" });
  assert.equal(cursor.command, "cursor-agent");
  assert.ok(cursor.args.includes("--model"));
  assert.ok(cursor.args.includes("gpt-5"));
  assert.ok(cursor.args.includes("--trust"));

  const kimi = await buildTaskCommand({ provider: "kimi", mode: "implement", prompt: "Implement", cwd, thinking: "high" });
  assert.equal(kimi.command, "pi");
  assert.ok(kimi.args.includes("--tools"));
  assert.ok(kimi.args.includes("read,bash,edit,write,grep,find,ls"));
  assert.ok(kimi.args.includes("--thinking"));

  const codex = await buildTaskCommand({ provider: "codex", mode: "review", prompt: "Review", cwd, thinking: "medium" });
  assert.equal(codex.command, "codex");
  assert.deepEqual(codex.args.slice(0, 3), ["exec", "--cd", cwd]);
  assert.ok(codex.args.includes("--json"));
});

test("builds Claude command through claude-p when explicitly configured", async () => {
  const cwd = process.cwd();
  const previous = process.env.CLAUDE_P_BIN;
  process.env.CLAUDE_P_BIN = "/custom/claude-p";
  try {
    const claude = await buildTaskCommand({ provider: "claude", mode: "review", prompt: "Review", cwd });
    assert.equal(claude.command, "/custom/claude-p");
    assert.deepEqual(claude.args.slice(0, 8), ["--cwd", cwd, "--timeout", "120", "--output-format", "json", "--permission-mode", "dontAsk"]);
  } finally {
    if (previous === undefined) {
      delete process.env.CLAUDE_P_BIN;
    } else {
      process.env.CLAUDE_P_BIN = previous;
    }
  }
});

test("builds native Claude command when CLAUDE_BIN is explicitly configured", async () => {
  const cwd = process.cwd();
  const previousClaude = process.env.CLAUDE_BIN;
  const previousClaudeP = process.env.CLAUDE_P_BIN;
  process.env.CLAUDE_BIN = "/custom/claude";
  delete process.env.CLAUDE_P_BIN;
  try {
    const claude = await buildTaskCommand({ provider: "claude", mode: "review", prompt: "Review", cwd });
    assert.equal(claude.command, "/custom/claude");
    assert.deepEqual(claude.args.slice(0, 5), ["-p", "--output-format", "json", "--permission-mode", "dontAsk"]);
    assert.ok(!claude.args.includes("--cwd"));
  } finally {
    if (previousClaude === undefined) {
      delete process.env.CLAUDE_BIN;
    } else {
      process.env.CLAUDE_BIN = previousClaude;
    }
    if (previousClaudeP === undefined) {
      delete process.env.CLAUDE_P_BIN;
    } else {
      process.env.CLAUDE_P_BIN = previousClaudeP;
    }
  }
});

test("clamps task timeout seconds", async () => {
  const low = await buildTaskCommand({ provider: "claude", mode: "review", prompt: "x", timeoutSeconds: 0 });
  const high = await buildTaskCommand({ provider: "claude", mode: "review", prompt: "x", timeoutSeconds: 9999 });
  assert.equal(low.timeoutSeconds, 1);
  assert.equal(high.timeoutSeconds, 1800);
});

test("task manager persists lifecycle state and captures logs", async () => {
  const stateDir = await tempDir();
  const manager = await createTaskManager({
    stateDir,
    defaultCwd: process.cwd(),
    spawnProcess: () => fakeChild({ stdout: "done", stderr: "warn", exitCode: 0 })
  });

  const task = await manager.spawn({ provider: "kimi", mode: "review", prompt: "Review", cwd: process.cwd(), title: "review task" });
  assert.equal(task.status, "running");
  const status = await waitForStatus(manager, task.taskId, "succeeded");
  assert.equal(status.status, "succeeded");
  assert.equal(status.title, "review task");

  const logs = await manager.logs(task.taskId);
  assert.equal(logs.stdout, "done");
  assert.equal(logs.stderr, "warn");

  const registry = await waitForRegistryStatus(stateDir, task.taskId, "succeeded");
  assert.equal(registry.tasks[task.taskId].status, "succeeded");
});

test("task manager records failed processes and result metadata", async () => {
  const stateDir = await tempDir();
  const manager = await createTaskManager({
    stateDir,
    defaultCwd: process.cwd(),
    spawnProcess: () => fakeChild({ stderr: "boom", exitCode: 7 })
  });

  const task = await manager.spawn({ provider: "codex", mode: "review", prompt: "Review", cwd: process.cwd() });
  await waitForStatus(manager, task.taskId, "failed");

  const result = await manager.result(task.taskId);
  assert.equal(result.status, "failed");
  assert.equal(result.exitCode, 7);
  assert.match(result.stderr, /boom/);
  assert.ok(Array.isArray(result.changedFiles));
});

test("task_stop terminates running tasks", async () => {
  const stateDir = await tempDir();
  let child;
  const manager = await createTaskManager({
    stateDir,
    defaultCwd: process.cwd(),
    spawnProcess: () => {
      child = fakeChild({ stdout: "later", delayMs: 1000 });
      return child;
    }
  });

  const task = await manager.spawn({ provider: "claude", mode: "implement", prompt: "Implement", cwd: process.cwd() });
  const stopped = await manager.stop(task.taskId);
  assert.equal(stopped.status, "stopped");
  assert.deepEqual(child.killCalls, ["SIGTERM"]);
});

test("startup recovery marks stale running tasks failed_stale", async () => {
  const stateDir = await tempDir();
  await fs.mkdir(path.join(stateDir, "tasks", "task_stale"), { recursive: true });
  await fs.writeFile(
    path.join(stateDir, "registry.json"),
    JSON.stringify({
      tasks: {
        task_stale: {
          taskId: "task_stale",
          provider: "claude",
          mode: "implement",
          status: "running",
          pid: 99999999,
          cwd: process.cwd(),
          taskDir: path.join(stateDir, "tasks", "task_stale"),
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString()
        }
      }
    })
  );

  const manager = await createTaskManager({ stateDir, defaultCwd: process.cwd(), spawnProcess: () => fakeChild() });
  const status = await manager.status("task_stale");
  assert.equal(status.status, "failed_stale");
});

test("readCappedFile truncates large logs", async () => {
  const dir = await tempDir();
  const file = path.join(dir, "stdout.log");
  await fs.writeFile(file, "x".repeat(100));
  const result = await readCappedFile(file, 10);
  assert.equal(result.text.length, 10);
  assert.equal(result.truncated, true);
});

test("task_remove fails when managed worktree cleanup fails", async () => {
  const stateDir = await tempDir();
  const taskDir = path.join(stateDir, "tasks", "task_worktree");
  await fs.mkdir(taskDir, { recursive: true });
  await fs.writeFile(
    path.join(stateDir, "registry.json"),
    JSON.stringify({
      tasks: {
        task_worktree: {
          taskId: "task_worktree",
          provider: "codex",
          mode: "implement",
          status: "succeeded",
          cwd: process.cwd(),
          taskDir,
          isolation: "worktree",
          worktreeManaged: true,
          worktreePath: path.join(stateDir, "missing-worktree"),
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString()
        }
      }
    })
  );
  const manager = await createTaskManager({
    stateDir,
    defaultCwd: process.cwd(),
    runGit: async () => ({ ok: false, stderr: "cannot remove" }),
    spawnProcess: () => fakeChild()
  });

  await assert.rejects(manager.remove("task_worktree"), /cannot remove/);
  assert.equal((await manager.status("task_worktree")).status, "succeeded");
});

test("provider env preserves local CLI credentials and bins", () => {
  const previousTerm = process.env.TERM;
  const previousAnthropic = process.env.ANTHROPIC_API_KEY;
  const previousAnthropicBaseUrl = process.env.ANTHROPIC_BASE_URL;
  const previousCustom = process.env.CLAUDE_PROVIDER_CUSTOM_ENV;
  process.env.TERM = "xterm-256color";
  process.env.ANTHROPIC_API_KEY = "test-anthropic-key";
  process.env.ANTHROPIC_BASE_URL = "http://127.0.0.1:8787";
  process.env.CLAUDE_PROVIDER_CUSTOM_ENV = "preserved";
  try {
    const env = buildProviderEnv();
    const claudeEnv = buildProviderEnv("claude");
    assert.equal(env.TERM, "xterm-256color");
    assert.equal(env.PATH, process.env.PATH);
    assert.equal(env.ANTHROPIC_API_KEY, "test-anthropic-key");
    assert.equal(env.ANTHROPIC_BASE_URL, "http://127.0.0.1:8787");
    assert.equal(claudeEnv.ANTHROPIC_API_KEY, "test-anthropic-key");
    assert.equal(claudeEnv.ANTHROPIC_BASE_URL, undefined);
    assert.equal(claudeEnv.CLAUDE_PROVIDER_CUSTOM_ENV, "preserved");
  } finally {
    if (previousTerm === undefined) {
      delete process.env.TERM;
    } else {
      process.env.TERM = previousTerm;
    }
    if (previousAnthropic === undefined) {
      delete process.env.ANTHROPIC_API_KEY;
    } else {
      process.env.ANTHROPIC_API_KEY = previousAnthropic;
    }
    if (previousAnthropicBaseUrl === undefined) {
      delete process.env.ANTHROPIC_BASE_URL;
    } else {
      process.env.ANTHROPIC_BASE_URL = previousAnthropicBaseUrl;
    }
    if (previousCustom === undefined) {
      delete process.env.CLAUDE_PROVIDER_CUSTOM_ENV;
    } else {
      process.env.CLAUDE_PROVIDER_CUSTOM_ENV = previousCustom;
    }
  }
});
