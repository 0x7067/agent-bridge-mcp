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
  let closed = false;
  const closeOnce = (closeExitCode, closeSignal) => {
    if (closed) {
      return;
    }
    closed = true;
    child.emit("close", closeExitCode, closeSignal);
  };
  child.kill = (killSignal = "SIGTERM") => {
    child.killCalls.push(killSignal);
    setTimeout(() => closeOnce(null, killSignal), 0);
    return true;
  };
  setTimeout(() => closeOnce(exitCode, signal), delayMs);
  return child;
}

function fakeErrorChild(error = new Error("spawn failed")) {
  const child = new EventEmitter();
  child.pid = undefined;
  child.stdout = Readable.from([]);
  child.stderr = Readable.from([]);
  child.kill = () => true;
  setTimeout(() => child.emit("error", error), 0);
  return child;
}

async function waitForStatus(manager, taskId, expectedStatus) {
  const deadline = Date.now() + 2000;
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
    "providers_check",
    "task_preview",
    "task_spawn",
    "task_list",
    "task_status",
    "task_wait",
    "task_logs",
    "task_result",
    "task_stop",
    "task_remove"
  ]);
  const taskLogs = response.result.tools.find((tool) => tool.name === "task_logs");
  assert.ok(taskLogs.inputSchema.properties.stdoutLine);
  assert.ok(taskLogs.inputSchema.properties.stderrLine);
  const providersCheck = response.result.tools.find((tool) => tool.name === "providers_check");
  assert.ok(providersCheck.inputSchema.properties.smoke);
  assert.ok(providersCheck.inputSchema.properties.timeoutMs);
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

test("tools/call accepts task_logs line cursor arguments", async () => {
  const response = await handleRequest({
    jsonrpc: "2.0",
    id: 4,
    method: "tools/call",
    params: { name: "task_logs", arguments: { taskId: "task_missing", stdoutLine: 1, stderrLine: 2 } }
  });
  assert.equal(response.result.isError, true);
  assert.match(response.result.content[0].text, /Unknown task/);
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

  assert.equal(claude.command, "/bin/zsh");
  assert.equal(claude.args[0], "-lc");
  assert.ok(claude.args.includes("claude-p"));
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
  assert.ok(codex.args.includes("shell_environment_policy.inherit=\"all\""));
});

test("builds Codex command with shell environment inheritance", async () => {
  const cwd = process.cwd();
  const codex = await buildTaskCommand({ provider: "codex", mode: "command", prompt: "Diagnose", cwd });
  const configIndex = codex.args.indexOf("--config");
  assert.notEqual(configIndex, -1);
  assert.equal(codex.args[configIndex + 1], "shell_environment_policy.inherit=\"all\"");
});

test("builds Claude claude-p command through shell init", async () => {
  const cwd = process.cwd();
  const previousClaudeP = process.env.CLAUDE_P_BIN;
  process.env.CLAUDE_P_BIN = "/custom/claude-p";
  try {
    const claude = await buildTaskCommand({ provider: "claude", mode: "command", prompt: "Review", cwd });
    assert.equal(claude.command, "/bin/zsh");
    assert.equal(claude.args[0], "-lc");
    assert.match(claude.args[1], /source ~\/\.zshenv/);
    assert.ok(claude.args.includes("/custom/claude-p"));
    assert.ok(claude.args.includes("--cwd"));
  } finally {
    if (previousClaudeP === undefined) {
      delete process.env.CLAUDE_P_BIN;
    } else {
      process.env.CLAUDE_P_BIN = previousClaudeP;
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
    assert.equal(claude.command, "/bin/zsh");
    assert.ok(claude.args.includes("/custom/claude"));
    assert.ok(claude.args.includes("-p"));
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

test("task_preview validates spawn args and redacts prompt", async () => {
  const response = await handleRequest({
    jsonrpc: "2.0",
    id: 10,
    method: "tools/call",
    params: {
      name: "task_preview",
      arguments: {
        provider: "codex",
        mode: "review",
        prompt: "secret prompt body",
        cwd: process.cwd(),
        thinking: "medium"
      }
    }
  });

  assert.equal(response.result.isError, false);
  const payload = JSON.parse(response.result.content[0].text);
  assert.equal(payload.command, "codex");
  assert.equal(payload.cwd, process.cwd());
  assert.equal(payload.timeoutSeconds, 120);
  assert.ok(payload.args.includes("<prompt redacted>"));
  assert.ok(!payload.args.includes("secret prompt body"));
  assert.ok(Array.isArray(payload.envKeys));
});

test("providers_check reports available and unavailable providers", async () => {
  const manager = await createTaskManager({
    stateDir: await tempDir(),
    defaultCwd: process.cwd(),
    providerBins: {
      claudeP: "claude-ok",
      cursor: "cursor-missing",
      kimi: "kimi-ok",
      codex: "codex-ok"
    },
    spawnProcess: (command) => {
      if (command === "cursor-missing") {
        return fakeErrorChild(new Error("ENOENT"));
      }
      return fakeChild({ stdout: `${command} 1.2.3\n`, exitCode: 0 });
    }
  });

  const result = await manager.checkProviders();
  assert.equal(result.providers.claude.available, true);
  assert.equal(result.providers.claude.command, "claude-ok");
  assert.match(result.providers.claude.version, /1\.2\.3/);
  assert.equal(result.providers.claude.probe, "version");
  assert.equal(result.providers.claude.startupVerified, false);
  assert.equal(result.providers.cursor.available, false);
  assert.equal(result.providers.cursor.command, "cursor-missing");
  assert.match(result.providers.cursor.error, /ENOENT/);
});

test("providers_check can run startup smoke probes", async () => {
  const calls = [];
  const manager = await createTaskManager({
    stateDir: await tempDir(),
    defaultCwd: process.cwd(),
    providerBins: {
      claudeP: "claude-ok",
      cursor: "cursor-ok",
      kimi: "kimi-ok",
      codex: "codex-ok"
    },
    spawnProcess: (command, args) => {
      calls.push({ command, args });
      if (args.includes("--version")) {
        return fakeChild({ stdout: `${command} version\n`, exitCode: 0 });
      }
      if (command === "kimi-ok") {
        return fakeChild({ stderr: "startup failed", exitCode: 2 });
      }
      return fakeChild({ stdout: "AGENT_BRIDGE_PROVIDER_SMOKE_OK\n", exitCode: 0 });
    }
  });

  const result = await manager.checkProviders({ smoke: true, timeoutMs: 1000 });
  assert.equal(result.providers.claude.available, true);
  assert.equal(result.providers.claude.probe, "version+smoke");
  assert.equal(result.providers.claude.startupVerified, true);
  assert.equal(result.providers.kimi.available, false);
  assert.equal(result.providers.kimi.startupVerified, false);
  assert.match(result.providers.kimi.error, /startup failed/);
  assert.ok(calls.some((call) => call.command === "codex-ok" && call.args.some((arg) => arg.includes("AGENT_BRIDGE_PROVIDER_SMOKE_OK"))));
});

test("providers_check mirrors native Claude bin selection", async () => {
  const previousClaude = process.env.CLAUDE_BIN;
  const previousClaudeP = process.env.CLAUDE_P_BIN;
  process.env.CLAUDE_BIN = "/custom/claude";
  delete process.env.CLAUDE_P_BIN;
  try {
    const manager = await createTaskManager({
      stateDir: await tempDir(),
      defaultCwd: process.cwd(),
      providerBins: {
        cursor: "cursor-ok",
        kimi: "kimi-ok",
        codex: "codex-ok"
      },
      spawnProcess: (command) => fakeChild({ stdout: `${command} version\n`, exitCode: 0 })
    });

    const result = await manager.checkProviders();
    assert.equal(result.providers.claude.command, "/custom/claude");
    assert.equal(result.providers.claude.available, true);
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
  assert.equal(status.isFinal, true);
  assert.equal(status.phase, "done");
  assert.equal(typeof status.durationMs, "number");

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
  assert.equal(result.errorType, "provider_exit_error");
  assert.match(result.stderr, /boom/);
  assert.ok(Array.isArray(result.changedFiles));
});

test("task manager records timeout and provider start error types", async () => {
  const timeoutManager = await createTaskManager({
    stateDir: await tempDir(),
    defaultCwd: process.cwd(),
    spawnProcess: () => fakeChild({ stdout: "later", delayMs: 1500 })
  });
  const timeoutTask = await timeoutManager.spawn({ provider: "codex", mode: "review", prompt: "Review", cwd: process.cwd(), timeoutSeconds: 1 });
  await waitForStatus(timeoutManager, timeoutTask.taskId, "failed");
  assert.equal((await timeoutManager.result(timeoutTask.taskId)).errorType, "timeout");

  const errorManager = await createTaskManager({
    stateDir: await tempDir(),
    defaultCwd: process.cwd(),
    spawnProcess: () => fakeErrorChild(new Error("missing binary"))
  });
  const errorTask = await errorManager.spawn({ provider: "kimi", mode: "review", prompt: "Review", cwd: process.cwd() });
  await waitForStatus(errorManager, errorTask.taskId, "failed");
  assert.equal((await errorManager.result(errorTask.taskId)).errorType, "provider_start_error");
});

test("task_wait resolves completed tasks and times out running tasks", async () => {
  const stateDir = await tempDir();
  const manager = await createTaskManager({
    stateDir,
    defaultCwd: process.cwd(),
    spawnProcess: () => fakeChild({ stdout: "done", exitCode: 0, delayMs: 20 })
  });

  const task = await manager.spawn({ provider: "kimi", mode: "review", prompt: "Review", cwd: process.cwd() });
  const completed = await manager.wait(task.taskId, 1000);
  assert.equal(completed.status, "succeeded");
  assert.equal(completed.isFinal, true);
  assert.equal(completed.timedOut, undefined);

  const slowManager = await createTaskManager({
    stateDir: await tempDir(),
    defaultCwd: process.cwd(),
    spawnProcess: () => fakeChild({ stdout: "later", delayMs: 1000 })
  });
  const slowTask = await slowManager.spawn({ provider: "claude", mode: "review", prompt: "Review", cwd: process.cwd() });
  const waiting = await slowManager.wait(slowTask.taskId, 5);
  assert.equal(waiting.status, "running");
  assert.equal(waiting.phase, "active");
  assert.equal(waiting.isFinal, false);
  assert.equal(waiting.timedOut, true);
  await slowManager.stop(slowTask.taskId);
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
  assert.equal(stopped.errorType, "stopped");
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
  assert.equal(status.errorType, "stale");
});

test("readCappedFile truncates large logs", async () => {
  const dir = await tempDir();
  const file = path.join(dir, "stdout.log");
  await fs.writeFile(file, "x".repeat(100));
  const result = await readCappedFile(file, 10);
  assert.equal(result.text.length, 10);
  assert.equal(result.truncated, true);
});

test("task_logs supports line cursors while preserving full reads", async () => {
  const stateDir = await tempDir();
  const manager = await createTaskManager({
    stateDir,
    defaultCwd: process.cwd(),
    spawnProcess: () => fakeChild({ stdout: "one\ntwo\nthree\n", stderr: "warn\nagain\n", exitCode: 0 })
  });

  const task = await manager.spawn({ provider: "kimi", mode: "review", prompt: "Review", cwd: process.cwd() });
  await waitForStatus(manager, task.taskId, "succeeded");

  const full = await manager.logs(task.taskId);
  assert.equal(full.stdout, "one\ntwo\nthree\n");
  assert.equal(full.stderr, "warn\nagain\n");
  assert.equal(full.nextStdoutLine, 3);
  assert.equal(full.nextStderrLine, 2);

  const incremental = await manager.logs(task.taskId, undefined, { stdoutLine: 1, stderrLine: 1 });
  assert.equal(incremental.stdout, "two\nthree\n");
  assert.equal(incremental.stderr, "again\n");
  assert.equal(incremental.nextStdoutLine, 3);
  assert.equal(incremental.nextStderrLine, 2);
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
    assert.equal(claudeEnv.CLAUDE_PROVIDER_CUSTOM_ENV, undefined);
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
