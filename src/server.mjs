#!/usr/bin/env node
import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import fsSync from "node:fs";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import readline from "node:readline";
import { fileURLToPath } from "node:url";
import {
  buildProviderCommand,
  buildProviderEnv,
  buildProviderSmokeCommand,
  buildProviderVersionCommand,
  getProviderCapabilities,
  hasProvider,
  providerNames,
  providerTaskModes,
  validateProviderTaskOptions
} from "./provider-registry.mjs";

export { buildProviderEnv } from "./provider-registry.mjs";

const PROTOCOL_VERSION = "2024-11-05";
const DEFAULT_TIMEOUT_SECONDS = 120;
const MAX_TIMEOUT_SECONDS = 1800;
const MIN_TIMEOUT_SECONDS = 1;
const MAX_PROMPT_BYTES = 100 * 1024;
const MAX_LOG_BYTES = 1024 * 1024;
const MAX_WAIT_MS = 60000;
const DEFAULT_PROVIDER_CHECK_TIMEOUT_MS = 5000;
const STARTUP_CWD = process.cwd();

const TASK_STATES = new Set(["queued", "running", "succeeded", "failed", "stopped", "failed_stale", "removed"]);
const TOOL_NAMES = ["providers_list", "providers_check", "task_preview", "task_spawn", "task_list", "task_status", "task_wait", "task_logs", "task_result", "task_stop", "task_remove"];
const FINAL_STATES = new Set(["succeeded", "failed", "stopped", "failed_stale"]);
const activeChildren = new Set();

const COMMON_SPAWN_FIELDS = new Set([
  "provider",
  "mode",
  "prompt",
  "title",
  "cwd",
  "timeoutSeconds",
  "model",
  "effort",
  "thinking",
  "isolation",
  "worktreeName"
]);

const TOOL_ARGUMENT_FIELDS = {
  providers_list: new Set(),
  providers_check: new Set(["smoke", "timeoutMs"]),
  task_preview: COMMON_SPAWN_FIELDS,
  task_spawn: COMMON_SPAWN_FIELDS,
  task_list: new Set(),
  task_status: new Set(["taskId"]),
  task_wait: new Set(["taskId", "timeoutMs"]),
  task_logs: new Set(["taskId", "maxBytes", "stdoutLine", "stderrLine"]),
  task_result: new Set(["taskId", "maxBytes"]),
  task_stop: new Set(["taskId"]),
  task_remove: new Set(["taskId"])
};

const toolDefinitions = [
  {
    name: "providers_list",
    description: "List first-class delegation providers and their task capabilities.",
    inputSchema: objectSchema({})
  },
  {
    name: "providers_check",
    description: "Check availability of each provider by running their command with --version, optionally with a startup smoke probe.",
    inputSchema: objectSchema({ smoke: { type: "boolean" }, timeoutMs: { type: "number" } })
  },
  {
    name: "task_preview",
    description: "Preview the command that would be run for a task without actually spawning it.",
    inputSchema: objectSchema({
      provider: { type: "string", enum: providerNames() },
      mode: { type: "string", enum: providerTaskModes() },
      prompt: { type: "string", description: "Task prompt. Maximum 100 KiB UTF-8." },
      title: { type: "string" },
      cwd: { type: "string", description: "Workspace directory under the allowed root." },
      timeoutSeconds: { type: "number" },
      model: { type: "string" },
      effort: { type: "string" },
      thinking: { type: "string" },
      isolation: { type: "string", enum: ["none", "worktree"] },
      worktreeName: { type: "string" }
    }, ["provider", "mode", "prompt"])
  },
  {
    name: "task_spawn",
    description: "Start a background provider task. Returns immediately; poll task_status/task_logs/task_result using the returned taskId.",
    inputSchema: objectSchema({
      provider: { type: "string", enum: providerNames() },
      mode: { type: "string", enum: providerTaskModes() },
      prompt: { type: "string", description: "Task prompt. Maximum 100 KiB UTF-8." },
      title: { type: "string" },
      cwd: { type: "string", description: "Workspace directory under the allowed root." },
      timeoutSeconds: { type: "number" },
      model: { type: "string" },
      effort: { type: "string" },
      thinking: { type: "string" },
      isolation: { type: "string", enum: ["none", "worktree"] },
      worktreeName: { type: "string" }
    }, ["provider", "mode", "prompt"])
  },
  {
    name: "task_list",
    description: "List tracked provider tasks.",
    inputSchema: objectSchema({})
  },
  {
    name: "task_status",
    description: "Read one task's lifecycle state.",
    inputSchema: objectSchema({ taskId: { type: "string" } }, ["taskId"])
  },
  {
    name: "task_wait",
    description: "Wait for a task to reach a final state or timeout.",
    inputSchema: objectSchema({ taskId: { type: "string" }, timeoutMs: { type: "number" } }, ["taskId"])
  },
  {
    name: "task_logs",
    description: "Return capped stdout/stderr log slices for a task.",
    inputSchema: objectSchema({
      taskId: { type: "string" },
      maxBytes: { type: "number" },
      stdoutLine: { type: "number" },
      stderrLine: { type: "number" }
    }, ["taskId"])
  },
  {
    name: "task_result",
    description: "Return final task metadata, logs, git status, diff, changed files, and exit metadata.",
    inputSchema: objectSchema({ taskId: { type: "string" }, maxBytes: { type: "number" } }, ["taskId"])
  },
  {
    name: "task_stop",
    description: "Terminate a running task.",
    inputSchema: objectSchema({ taskId: { type: "string" } }, ["taskId"])
  },
  {
    name: "task_remove",
    description: "Remove a finished/stopped task. Managed worktree cleanup is mandatory and failure keeps the task record.",
    inputSchema: objectSchema({ taskId: { type: "string" } }, ["taskId"])
  }
];

let defaultTaskManagerPromise;

function objectSchema(properties, required = []) {
  return { type: "object", additionalProperties: false, required, properties };
}

export async function handleRequest(request) {
  if (!request || request.jsonrpc !== "2.0" || typeof request.method !== "string") {
    return jsonRpcError(request?.id ?? null, -32600, "Invalid Request");
  }

  if (!Object.hasOwn(request, "id")) {
    await handleNotification(request);
    return undefined;
  }

  try {
    switch (request.method) {
      case "initialize":
        return jsonRpcResult(request.id, {
          protocolVersion: PROTOCOL_VERSION,
          capabilities: { tools: {} },
          serverInfo: { name: "agent-bridge-mcp", version: "0.1.0" }
        });
      case "tools/list":
        return jsonRpcResult(request.id, { tools: toolDefinitions });
      case "tools/call":
        return jsonRpcResult(request.id, await callTool(request.params ?? {}));
      default:
        return jsonRpcError(request.id, -32601, `Method not found: ${request.method}`);
    }
  } catch (error) {
    return jsonRpcError(request.id, -32603, error instanceof Error ? error.message : String(error));
  }
}

async function handleNotification(request) {
  if (request.method === "notifications/initialized") {
    return;
  }
}

async function callTool(params) {
  const name = params.name;
  const args = params.arguments ?? {};

  if (!TOOL_NAMES.includes(name)) {
    return toolError(`Unknown tool: ${name}`);
  }

  try {
    rejectUnknownToolArguments(name, args);

    if (name === "providers_list") {
      return toolJson({ providers: getProviderCapabilities() });
    }

    const manager = await getDefaultTaskManager();
    switch (name) {
      case "providers_check":
        return toolJson(await manager.checkProviders(args));
      case "task_preview":
        return toolJson(await manager.preview(args));
      case "task_spawn":
        return toolJson(await manager.spawn(args));
      case "task_list":
        return toolJson(await manager.list());
      case "task_status":
        return toolJson(await manager.status(requireTaskId(args)));
      case "task_wait":
        return toolJson(await manager.wait(requireTaskId(args), args.timeoutMs));
      case "task_logs":
        return toolJson(await manager.logs(requireTaskId(args), args.maxBytes, { stdoutLine: args.stdoutLine, stderrLine: args.stderrLine }));
      case "task_result":
        return toolJson(await manager.result(requireTaskId(args), args.maxBytes));
      case "task_stop":
        return toolJson(await manager.stop(requireTaskId(args)));
      case "task_remove":
        return toolJson(await manager.remove(requireTaskId(args)));
      default:
        return toolError(`Unknown tool: ${name}`);
    }
  } catch (error) {
    return toolError(error instanceof Error ? error.message : String(error));
  }
}

function rejectUnknownToolArguments(toolName, args) {
  const allowedFields = TOOL_ARGUMENT_FIELDS[toolName];
  for (const field of Object.keys(args ?? {})) {
    if (!allowedFields.has(field)) {
      throw new Error(`Unknown argument for ${toolName}: ${field}`);
    }
  }
}

function requireTaskId(args) {
  if (!args || typeof args.taskId !== "string" || args.taskId.length === 0) {
    throw new Error("taskId is required");
  }
  return args.taskId;
}

async function getDefaultTaskManager() {
  defaultTaskManagerPromise ??= createTaskManager();
  return defaultTaskManagerPromise;
}

export async function validateTaskSpawnArguments(input = {}, options = {}) {
  for (const field of Object.keys(input)) {
    if (!COMMON_SPAWN_FIELDS.has(field)) {
      throw new Error(`Unknown argument for task_spawn: ${field}`);
    }
  }

  const provider = input.provider;
  if (!hasProvider(provider)) {
    throw new Error(`provider must be one of: ${providerNames().join(", ")}`);
  }

  const mode = input.mode;
  validateProviderTaskOptions({ provider, mode, effort: input.effort, thinking: input.thinking });

  const prompt = input.prompt;
  if (typeof prompt !== "string" || prompt.length === 0) {
    throw new Error("prompt is required");
  }
  if (Buffer.byteLength(prompt, "utf8") > MAX_PROMPT_BYTES) {
    throw new Error(`prompt exceeds ${MAX_PROMPT_BYTES} bytes`);
  }

  if (input.title !== undefined && typeof input.title !== "string") {
    throw new Error("title must be a string");
  }
  if (input.model !== undefined && typeof input.model !== "string") {
    throw new Error("model must be a string");
  }

  const isolation = input.isolation ?? "none";
  if (!["none", "worktree"].includes(isolation)) {
    throw new Error("isolation must be one of: none, worktree");
  }
  if (input.worktreeName !== undefined && !/^[A-Za-z0-9._-]+$/.test(input.worktreeName)) {
    throw new Error("worktreeName may contain only letters, numbers, dot, underscore, and hyphen");
  }

  const cwd = await resolveSafeCwd(input.cwd, options);
  return {
    provider,
    mode,
    prompt,
    title: input.title,
    cwd,
    timeoutSeconds: clampTimeoutSeconds(input.timeoutSeconds),
    model: input.model,
    effort: input.effort,
    thinking: input.thinking,
    isolation,
    worktreeName: input.worktreeName
  };
}

export async function buildTaskCommand(input = {}, options = {}) {
  const task = await validateTaskSpawnArguments(input, options);
  return buildProviderCommand(task, { providerBins: options.providerBins });
}

export async function createTaskManager(options = {}) {
  const stateDir = expandHome(options.stateDir ?? process.env.AGENT_BRIDGE_STATE_DIR ?? "~/.agent-bridge-mcp/state");
  const manager = new TaskManager({
    stateDir,
    defaultCwd: options.defaultCwd ?? STARTUP_CWD,
    allowedRoot: options.allowedRoot,
    spawnProcess: options.spawnProcess,
    runGit: options.runGit,
    providerBins: options.providerBins
  });
  await manager.init();
  return manager;
}

class TaskManager {
  constructor(options) {
    this.stateDir = options.stateDir;
    this.defaultCwd = options.defaultCwd;
    this.allowedRoot = options.allowedRoot;
    this.spawnProcess = options.spawnProcess ?? spawn;
    this.runGit = options.runGit ?? runGitCommand;
    this.providerBins = options.providerBins;
    this.registry = { tasks: {} };
    this.active = new Map();
    this.saveQueue = Promise.resolve();
  }

  async init() {
    await fs.mkdir(path.join(this.stateDir, "tasks"), { recursive: true });
    this.registry = await loadRegistry(this.stateDir);
    let changed = false;
    for (const task of Object.values(this.registry.tasks)) {
      if (task.status === "running" || task.status === "queued") {
        task.status = "failed_stale";
        task.error = "task was running when the MCP server restarted; resume is not supported in v1";
        task.updatedAt = nowIso();
        changed = true;
      }
    }
    if (changed) {
      await this.save();
    }
  }

  async preview(input) {
    const validated = await validateTaskSpawnArguments(input, {
      allowedRoot: this.allowedRoot,
      defaultCwd: this.defaultCwd
    });
    const command = await buildTaskCommand(validated, { providerBins: this.providerBins });
    const redactedArgs = command.args.map((arg) => {
      if (arg.includes(validated.prompt)) return "<prompt redacted>";
      return arg;
    });
    const env = buildCommandEnv(validated.provider, command);
    return {
      command: command.command,
      cwd: command.cwd,
      timeoutSeconds: command.timeoutSeconds,
      args: redactedArgs,
      envKeys: Object.keys(env)
    };
  }

  async checkProviders(options = {}) {
    const smoke = options.smoke === true;
    const timeoutMs = normalizeProviderCheckTimeoutMs(options.timeoutMs);
    const results = {};
    for (const name of providerNames()) {
      const versionCommand = buildProviderVersionCommand(name, { providerBins: this.providerBins });
      const versionResult = await runCommand(versionCommand.command, versionCommand.args, {
        cwd: this.defaultCwd,
        env: versionCommand.env,
        timeoutMs,
        maxBufferBytes: 8192,
        spawnProcess: this.spawnProcess
      });
      if (!versionResult.ok) {
        results[name] = {
          available: false,
          command: versionCommand.command,
          probe: "version",
          startupVerified: false,
          error: versionResult.stderr.trim() || versionResult.error || `exited with code ${versionResult.exitCode}`
        };
        continue;
      }

      const baseResult = {
        available: true,
        command: versionCommand.command,
        version: versionResult.stdout.trim(),
        probe: smoke ? "version+smoke" : "version",
        startupVerified: false
      };
      if (!smoke) {
        results[name] = baseResult;
        continue;
      }

      const smokeCommand = this.providerSmokeCommand(name, timeoutMs);
      const smokeResult = await runCommand(smokeCommand.command, smokeCommand.args, {
        cwd: smokeCommand.cwd,
        env: buildCommandEnv(name, smokeCommand),
        timeoutMs,
        maxBufferBytes: 8192,
        spawnProcess: this.spawnProcess
      });
      results[name] = smokeResult.ok
        ? { ...baseResult, startupVerified: true }
        : {
            ...baseResult,
            available: false,
            error: smokeResult.stderr.trim() || smokeResult.error || `exited with code ${smokeResult.exitCode}`
          };
    }
    return { providers: results };
  }

  async wait(taskId, timeoutMs = 30000) {
    const task = this.requireTask(taskId);
    const deadline = Date.now() + normalizeWaitMs(timeoutMs);
    while (Date.now() < deadline) {
      if (FINAL_STATES.has(task.status)) {
        return { ...this.publicTask(task), timedOut: undefined };
      }
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    return { ...this.publicTask(task), timedOut: true };
  }

  async spawn(input) {
    const validated = await validateTaskSpawnArguments(input, {
      allowedRoot: this.allowedRoot,
      defaultCwd: this.defaultCwd
    });
    const taskId = `task_${randomUUID().replaceAll("-", "")}`;
    const createdAt = nowIso();
    const taskDir = path.join(this.stateDir, "tasks", taskId);
    await fs.mkdir(taskDir, { recursive: true });

    let cwd = validated.cwd;
    let worktreePath;
    if (validated.isolation === "worktree") {
      const worktree = await this.createWorktree(validated, taskId);
      cwd = worktree.worktreePath;
      worktreePath = worktree.worktreePath;
    }

    const command = await buildTaskCommand({ ...validated, cwd }, { providerBins: this.providerBins });
    const record = {
      taskId,
      provider: validated.provider,
      mode: validated.mode,
      title: validated.title,
      status: "queued",
      cwd,
      originalCwd: validated.cwd,
      isolation: validated.isolation,
      worktreeManaged: validated.isolation === "worktree",
      worktreePath,
      taskDir,
      command: command.command,
      args: command.args,
      timeoutSeconds: validated.timeoutSeconds,
      createdAt,
      updatedAt: createdAt
    };
    this.registry.tasks[taskId] = record;
    await this.save();

    this.launch(record, command);
    return this.publicTask(record);
  }

  launch(record, command) {
    const stdoutPath = path.join(record.taskDir, "stdout.log");
    const stderrPath = path.join(record.taskDir, "stderr.log");
    const child = this.spawnProcess(command.command, command.args, {
      cwd: command.cwd,
      env: buildCommandEnv(record.provider, command),
      stdio: ["ignore", "pipe", "pipe"]
    });
    activeChildren.add(child);
    record.pid = child.pid;
    record.status = "running";
    record.startedAt = nowIso();
    record.updatedAt = record.startedAt;
    this.save().catch((error) => this.logPersistError(record.taskId, error));

    let timedOut = false;
    let stopRequested = false;
    const timeout = setTimeout(() => {
      timedOut = true;
      child.kill("SIGTERM");
    }, record.timeoutSeconds * 1000);

    this.active.set(record.taskId, { child, timeout, markStopped: () => { stopRequested = true; } });

    child.stdout?.on("data", (chunk) => appendCappedLog(stdoutPath, chunk, MAX_LOG_BYTES));
    child.stderr?.on("data", (chunk) => appendCappedLog(stderrPath, chunk, MAX_LOG_BYTES));
    child.on("error", (error) => {
      clearTimeout(timeout);
      this.active.delete(record.taskId);
      activeChildren.delete(child);
      this.finish(record.taskId, { status: "failed", error: error.message, errorType: "provider_start_error" }).catch((finishError) => this.logPersistError(record.taskId, finishError));
    });
    child.on("close", (exitCode, signal) => {
      clearTimeout(timeout);
      this.active.delete(record.taskId);
      activeChildren.delete(child);
      let status = "succeeded";
      let error;
      if (stopRequested) {
        status = "stopped";
        error = `task stopped with signal ${signal ?? "SIGTERM"}`;
      } else if (timedOut) {
        status = "failed";
        error = `task timed out after ${record.timeoutSeconds * 1000}ms`;
      } else if (exitCode !== 0) {
        status = "failed";
        error = `command exited with code ${exitCode}`;
      }
      this.finish(record.taskId, { status, error, errorType: inferErrorType({ status, timedOut, stopRequested }), exitCode, signal }).catch((finishError) => this.logPersistError(record.taskId, finishError));
    });
  }

  async finish(taskId, patch) {
    const task = this.requireTask(taskId);
    Object.assign(task, patch, {
      completedAt: nowIso(),
      updatedAt: nowIso()
    });
    const snapshot = await this.gitSnapshot(task.cwd);
    task.gitStatus = snapshot.gitStatus;
    task.gitDiff = snapshot.gitDiff;
    task.changedFiles = snapshot.changedFiles;
    await fs.writeFile(path.join(task.taskDir, "result.json"), JSON.stringify(task, null, 2));
    await this.save();
  }

  async list() {
    return {
      tasks: Object.values(this.registry.tasks)
        .filter((task) => task.status !== "removed")
        .map((task) => this.publicTask(task))
    };
  }

  async status(taskId) {
    return this.publicTask(this.requireTask(taskId));
  }

  async logs(taskId, maxBytes = MAX_LOG_BYTES, options = {}) {
    const task = this.requireTask(taskId);
    const [stdout, stderr] = await Promise.all([
      readCappedFile(path.join(task.taskDir, "stdout.log"), normalizeMaxBytes(maxBytes)),
      readCappedFile(path.join(task.taskDir, "stderr.log"), normalizeMaxBytes(maxBytes))
    ]);
    const stdoutLine = options.stdoutLine ?? 0;
    const stderrLine = options.stderrLine ?? 0;
    const slicedStdout = sliceLines(stdout.text, stdoutLine);
    const slicedStderr = sliceLines(stderr.text, stderrLine);
    return {
      taskId,
      status: task.status,
      stdout: slicedStdout.text,
      stderr: slicedStderr.text,
      stdoutTruncated: stdout.truncated,
      stderrTruncated: stderr.truncated,
      nextStdoutLine: slicedStdout.nextLine,
      nextStderrLine: slicedStderr.nextLine
    };
  }

  async result(taskId, maxBytes = MAX_LOG_BYTES) {
    const task = this.requireTask(taskId);
    const logs = await this.logs(taskId, maxBytes);
    return {
      ...this.publicTask(task),
      exitCode: task.exitCode,
      signal: task.signal,
      error: task.error,
      stdout: logs.stdout,
      stderr: logs.stderr,
      stdoutTruncated: logs.stdoutTruncated,
      stderrTruncated: logs.stderrTruncated,
      gitStatus: task.gitStatus ?? "",
      gitDiff: task.gitDiff ?? "",
      changedFiles: task.changedFiles ?? []
    };
  }

  async stop(taskId) {
    const task = this.requireTask(taskId);
    const active = this.active.get(taskId);
    if (!active) {
      if (FINAL_STATES.has(task.status)) {
        return this.publicTask(task);
      }
      throw new Error(`task is not running: ${taskId}`);
    }
    active.markStopped();
    task.status = "stopped";
    task.updatedAt = nowIso();
    await this.save();
    active.child.kill("SIGTERM");
    return this.publicTask(task);
  }

  async remove(taskId) {
    const task = this.requireTask(taskId);
    if (task.status === "running" || task.status === "queued") {
      throw new Error("cannot remove a running task; stop it first");
    }

    if (task.worktreeManaged && task.worktreePath) {
      const result = await this.runGit(["worktree", "remove", "-f", task.worktreePath], { cwd: task.originalCwd ?? task.cwd });
      if (!result.ok) {
        throw new Error(result.stderr || result.stdout || result.error || "failed to remove worktree");
      }
    }

    task.status = "removed";
    task.updatedAt = nowIso();
    await this.save();
    await fs.rm(task.taskDir, { recursive: true, force: true });
    return { taskId, status: "removed" };
  }

  async createWorktree(task, taskId) {
    const root = await this.gitOutput(["rev-parse", "--show-toplevel"], { cwd: task.cwd });
    const baseName = task.worktreeName ?? `${task.provider}-${task.mode}-${taskId.slice(-8)}`;
    const branchName = `agent-bridge/${baseName}`;
    const worktreeRoot = path.join(this.stateDir, "worktrees");
    const worktreePath = path.join(worktreeRoot, baseName);
    await fs.mkdir(worktreeRoot, { recursive: true });

    const result = await this.runGit(["worktree", "add", "-b", branchName, worktreePath], { cwd: root.trim() });
    if (!result.ok) {
      throw new Error(result.stderr || result.stdout || result.error || "failed to create worktree");
    }
    return { worktreePath };
  }

  async gitSnapshot(cwd) {
    const [status, diff, changed] = await Promise.all([
      this.runGit(["status", "--short"], { cwd }),
      this.runGit(["diff", "--"], { cwd }),
      this.runGit(["diff", "--name-only"], { cwd })
    ]);
    return {
      gitStatus: status.ok ? capText(status.stdout, MAX_LOG_BYTES).text : "",
      gitDiff: diff.ok ? capText(diff.stdout, MAX_LOG_BYTES).text : "",
      changedFiles: changed.ok ? changed.stdout.split(/\r?\n/).filter(Boolean) : []
    };
  }

  async gitOutput(args, options) {
    const result = await this.runGit(args, options);
    if (!result.ok) {
      throw new Error(result.stderr || result.stdout || result.error || `git ${args.join(" ")} failed`);
    }
    return result.stdout;
  }

  providerSmokeCommand(provider, timeoutMs) {
    return buildProviderSmokeCommand(provider, {
      provider,
      cwd: this.defaultCwd,
      timeoutSeconds: Math.max(MIN_TIMEOUT_SECONDS, Math.ceil(timeoutMs / 1000)),
      providerBins: this.providerBins
    });
  }

  publicTask(task) {
    const isFinal = FINAL_STATES.has(task.status);
    let phase;
    if (task.status === "queued") phase = "pending";
    else if (task.status === "running") phase = "active";
    else phase = "done";
    let durationMs;
    if (task.startedAt && task.completedAt) {
      durationMs = new Date(task.completedAt) - new Date(task.startedAt);
    } else if (task.startedAt && task.status === "running") {
      durationMs = Date.now() - new Date(task.startedAt);
    }
    const errorType = task.errorType ?? inferErrorType(task);
    return {
      taskId: task.taskId,
      provider: task.provider,
      mode: task.mode,
      title: task.title,
      status: task.status,
      cwd: task.cwd,
      isolation: task.isolation,
      worktreePath: task.worktreePath,
      pid: task.pid,
      createdAt: task.createdAt,
      updatedAt: task.updatedAt,
      startedAt: task.startedAt,
      completedAt: task.completedAt,
      isFinal,
      phase,
      durationMs,
      errorType
    };
  }

  requireTask(taskId) {
    const task = this.registry.tasks[taskId];
    if (!task || task.status === "removed") {
      throw new Error(`Unknown task: ${taskId}`);
    }
    if (!TASK_STATES.has(task.status)) {
      throw new Error(`Invalid task state for ${taskId}: ${task.status}`);
    }
    return task;
  }

  async save() {
    const pending = this.saveQueue.catch(() => {}).then(() => saveRegistry(this.stateDir, this.registry));
    this.saveQueue = pending;
    await pending;
  }

  logPersistError(taskId, error) {
    console.error(`[agent-bridge] failed to persist task=${taskId}: ${error instanceof Error ? error.message : error}`);
  }
}

export async function loadRegistry(stateDir) {
  const registryPath = path.join(stateDir, "registry.json");
  try {
    const text = await fs.readFile(registryPath, "utf8");
    const parsed = JSON.parse(text);
    return parsed && typeof parsed === "object" && parsed.tasks && typeof parsed.tasks === "object" ? parsed : { tasks: {} };
  } catch (error) {
    if (error?.code === "ENOENT") {
      return { tasks: {} };
    }
    throw error;
  }
}

async function saveRegistry(stateDir, registry) {
  await fs.mkdir(stateDir, { recursive: true });
  const registryPath = path.join(stateDir, "registry.json");
  const tmpPath = `${registryPath}.tmp-${process.pid}-${Date.now()}-${randomUUID()}`;
  await fs.writeFile(tmpPath, JSON.stringify(registry, null, 2));
  await fs.rename(tmpPath, registryPath);
}

async function resolveSafeCwd(cwdInput, options = {}) {
  const allowedRootInput = options.allowedRoot ?? process.env.AGENT_BRIDGE_ALLOWED_ROOT;
  const defaultCwd = options.defaultCwd ?? STARTUP_CWD;

  if (cwdInput !== undefined && typeof cwdInput !== "string") {
    throw new Error("cwd must be a string");
  }
  if (cwdInput && hasDotDotSegment(cwdInput)) {
    throw new Error("cwd must not contain .. segments");
  }

  const candidate = path.resolve(cwdInput || defaultCwd);
  const rootCandidate = path.resolve(allowedRootInput || defaultCwd);
  const [realCwd, realRoot] = await Promise.all([realDirectory(candidate, "cwd"), realDirectory(rootCandidate, "allowed root")]);

  if (!isPathInside(realCwd, realRoot)) {
    throw new Error(`cwd is outside allowed root: ${realRoot}`);
  }

  return realCwd;
}

async function realDirectory(candidate, label) {
  const stat = await fs.stat(candidate);
  if (!stat.isDirectory()) {
    throw new Error(`${label} is not a directory: ${candidate}`);
  }
  return fs.realpath(candidate);
}

function hasDotDotSegment(value) {
  return value.split(/[\\/]+/).includes("..");
}

function isPathInside(candidate, root) {
  const relative = path.relative(root, candidate);
  return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative));
}

function clampTimeoutSeconds(value) {
  const numeric = Number.isFinite(value) ? value : DEFAULT_TIMEOUT_SECONDS;
  return Math.min(MAX_TIMEOUT_SECONDS, Math.max(MIN_TIMEOUT_SECONDS, Math.trunc(numeric)));
}

function normalizeMaxBytes(value) {
  const numeric = Number.isFinite(value) ? value : MAX_LOG_BYTES;
  return Math.min(MAX_LOG_BYTES, Math.max(1, Math.trunc(numeric)));
}

function normalizeWaitMs(value) {
  const numeric = Number.isFinite(value) ? value : 30000;
  return Math.min(MAX_WAIT_MS, Math.max(0, Math.trunc(numeric)));
}

function normalizeProviderCheckTimeoutMs(value) {
  const numeric = Number.isFinite(value) ? value : DEFAULT_PROVIDER_CHECK_TIMEOUT_MS;
  return Math.min(MAX_WAIT_MS, Math.max(1, Math.trunc(numeric)));
}

export async function readCappedFile(file, maxBytes = MAX_LOG_BYTES) {
  try {
    const buffer = await fs.readFile(file);
    const capped = capBuffer(buffer, normalizeMaxBytes(maxBytes));
    return { text: capped.buffer.toString("utf8"), truncated: capped.truncated };
  } catch (error) {
    if (error?.code === "ENOENT") {
      return { text: "", truncated: false };
    }
    throw error;
  }
}

function appendCappedLog(file, chunk, maxBytes) {
  const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(String(chunk));
  fsSync.mkdirSync(path.dirname(file), { recursive: true });
  const currentSize = fsSync.existsSync(file) ? fsSync.statSync(file).size : 0;
  if (currentSize >= maxBytes) {
    return;
  }
  const remaining = maxBytes - currentSize;
  fsSync.appendFileSync(file, buffer.subarray(0, remaining));
}

function capText(text, maxBytes) {
  return capBuffer(Buffer.from(text, "utf8"), maxBytes);
}

function capBuffer(buffer, maxBytes) {
  if (buffer.byteLength <= maxBytes) {
    return { buffer, text: buffer.toString("utf8"), truncated: false };
  }
  const capped = buffer.subarray(0, maxBytes);
  return { buffer: capped, text: capped.toString("utf8"), truncated: true };
}

function sliceLines(text, startLine) {
  if (text === "") {
    return { text: "", nextLine: 0 };
  }
  const lines = text.split(/\r?\n/);
  const endsWithNewline = text.endsWith("\n") || text.endsWith("\r\n");
  const totalLines = endsWithNewline ? lines.length - 1 : lines.length;
  const endIndex = endsWithNewline ? lines.length - 1 : lines.length;
  const sliced = lines.slice(startLine, endIndex);
  let result = sliced.join("\n");
  if (endsWithNewline && sliced.length > 0) {
    result += "\n";
  }
  return { text: result, nextLine: totalLines };
}

function inferErrorType(task) {
  if (task.errorType) {
    return task.errorType;
  }
  if (task.timedOut) {
    return "timeout";
  }
  if (task.stopRequested || task.status === "stopped") {
    return "stopped";
  }
  if (task.status === "failed_stale") {
    return "stale";
  }
  if (task.status === "failed") {
    return "provider_exit_error";
  }
  return undefined;
}

function buildCommandEnv(provider, command) {
  return { ...buildProviderEnv(provider), ...(command.env ?? {}) };
}

export async function runCommand(command, args, options = {}) {
  const timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_SECONDS * 1000;
  const maxBufferBytes = options.maxBufferBytes ?? MAX_LOG_BYTES;

  return new Promise((resolve) => {
    let stdout = "";
    let stderr = "";
    let stdoutTruncated = false;
    let stderrTruncated = false;
    let timedOut = false;

    const spawnProcess = options.spawnProcess ?? spawn;
    const child = spawnProcess(command, args, {
      cwd: options.cwd,
      env: options.env ?? buildProviderEnv(),
      stdio: ["ignore", "pipe", "pipe"]
    });
    activeChildren.add(child);

    const timeout = setTimeout(() => {
      timedOut = true;
      child.kill("SIGTERM");
    }, timeoutMs);

    child.stdout?.on("data", (chunk) => {
      const captured = appendCapped(stdout, chunk, maxBufferBytes);
      stdout = captured.text;
      stdoutTruncated ||= captured.truncated;
    });
    child.stderr?.on("data", (chunk) => {
      const captured = appendCapped(stderr, chunk, maxBufferBytes);
      stderr = captured.text;
      stderrTruncated ||= captured.truncated;
    });
    child.on("error", (error) => {
      clearTimeout(timeout);
      activeChildren.delete(child);
      resolve({ ok: false, error: error.message, stdout, stderr, stdoutTruncated, stderrTruncated });
    });
    child.on("close", (exitCode, signal) => {
      clearTimeout(timeout);
      activeChildren.delete(child);
      if (timedOut) {
        resolve({ ok: false, error: `command timed out after ${timeoutMs}ms`, exitCode, signal, stdout, stderr });
        return;
      }
      if (exitCode !== 0) {
        resolve({ ok: false, error: `command exited with code ${exitCode}`, exitCode, signal, stdout, stderr });
        return;
      }
      resolve({ ok: true, exitCode, signal, stdout, stderr, stdoutTruncated, stderrTruncated });
    });
  });
}

function appendCapped(current, chunk, maxBufferBytes) {
  const next = current + chunk.toString("utf8");
  if (Buffer.byteLength(next, "utf8") <= maxBufferBytes) {
    return { text: next, truncated: false };
  }
  return {
    text: Buffer.from(next, "utf8").subarray(0, maxBufferBytes).toString("utf8"),
    truncated: true
  };
}

async function runGitCommand(args, options = {}) {
  return runCommand("git", args, { cwd: options.cwd, timeoutMs: 30000, maxBufferBytes: MAX_LOG_BYTES });
}

function expandHome(value) {
  if (value === "~") {
    return os.homedir();
  }
  if (value.startsWith("~/")) {
    return path.join(os.homedir(), value.slice(2));
  }
  return path.resolve(value);
}

function nowIso() {
  return new Date().toISOString();
}

function jsonRpcResult(id, result) {
  return { jsonrpc: "2.0", id, result };
}

function jsonRpcError(id, code, message) {
  return { jsonrpc: "2.0", id, error: { code, message } };
}

function toolJson(value) {
  return { content: [{ type: "text", text: JSON.stringify(value, null, 2) }], isError: false };
}

function toolError(text) {
  return { content: [{ type: "text", text }], isError: true };
}

function terminateChildren() {
  for (const child of activeChildren) {
    child.kill("SIGTERM");
  }
}

process.once("SIGINT", () => {
  terminateChildren();
  process.exit(130);
});
process.once("SIGTERM", () => {
  terminateChildren();
  process.exit(143);
});

export async function runStdioServer() {
  const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
  for await (const line of rl) {
    if (!line.trim()) {
      continue;
    }

    let request;
    try {
      request = JSON.parse(line);
    } catch {
      process.stdout.write(JSON.stringify(jsonRpcError(null, -32700, "Parse error")) + "\n");
      continue;
    }

    const response = await handleRequest(request);
    if (response !== undefined) {
      process.stdout.write(JSON.stringify(response) + "\n");
    }
  }
}

const thisFile = fileURLToPath(import.meta.url);
if (process.argv[1] === thisFile) {
  runStdioServer().catch((error) => {
    console.error(`[agent-bridge] fatal ${error instanceof Error ? error.stack : error}`);
    process.exit(1);
  });
}
