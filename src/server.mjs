#!/usr/bin/env node
import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import fsSync from "node:fs";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import readline from "node:readline";
import { fileURLToPath } from "node:url";

const PROTOCOL_VERSION = "2024-11-05";
const DEFAULT_TIMEOUT_SECONDS = 120;
const MAX_TIMEOUT_SECONDS = 1800;
const MIN_TIMEOUT_SECONDS = 1;
const MAX_PROMPT_BYTES = 100 * 1024;
const MAX_LOG_BYTES = 1024 * 1024;
const STARTUP_CWD = process.cwd();

const TASK_STATES = new Set(["queued", "running", "succeeded", "failed", "stopped", "failed_stale", "removed"]);
const TOOL_NAMES = ["providers_list", "task_spawn", "task_list", "task_status", "task_logs", "task_result", "task_stop", "task_remove"];
const FINAL_STATES = new Set(["succeeded", "failed", "stopped", "failed_stale"]);
const activeChildren = new Set();

const PROVIDERS = {
  claude: {
    modes: ["research", "review", "implement", "command"],
    supportsReply: false,
    supportsResume: false,
    supportsWorktreeIsolation: true,
    effort: ["low", "medium", "high", "xhigh", "max"]
  },
  cursor: {
    modes: ["research", "review", "implement"],
    supportsReply: false,
    supportsResume: false,
    supportsWorktreeIsolation: true
  },
  kimi: {
    modes: ["research", "review", "implement", "command"],
    supportsReply: false,
    supportsResume: false,
    supportsWorktreeIsolation: true,
    thinking: ["off", "minimal", "low", "medium", "high", "xhigh"]
  },
  codex: {
    modes: ["research", "review", "implement", "command"],
    supportsReply: false,
    supportsResume: false,
    supportsWorktreeIsolation: true,
    thinking: ["low", "medium", "high", "xhigh"]
  }
};

const MODE_DESCRIPTIONS = {
  research: "Read and analyze. Do not edit files.",
  review: "Review the requested code or plan. Do not edit files.",
  implement: "Make the requested code changes, keep scope tight, and report verification evidence.",
  command: "Run the requested bounded command-oriented task and report evidence."
};

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
  task_spawn: COMMON_SPAWN_FIELDS,
  task_list: new Set(),
  task_status: new Set(["taskId"]),
  task_logs: new Set(["taskId", "maxBytes"]),
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
    name: "task_spawn",
    description: "Start a background provider task. Returns immediately; poll task_status/task_logs/task_result using the returned taskId.",
    inputSchema: objectSchema({
      provider: { type: "string", enum: Object.keys(PROVIDERS) },
      mode: { type: "string", enum: ["research", "review", "implement", "command"] },
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
    name: "task_logs",
    description: "Return capped stdout/stderr log slices for a task.",
    inputSchema: objectSchema({ taskId: { type: "string" }, maxBytes: { type: "number" } }, ["taskId"])
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
      return toolJson({ providers: PROVIDERS });
    }

    const manager = await getDefaultTaskManager();
    switch (name) {
      case "task_spawn":
        return toolJson(await manager.spawn(args));
      case "task_list":
        return toolJson(await manager.list());
      case "task_status":
        return toolJson(await manager.status(requireTaskId(args)));
      case "task_logs":
        return toolJson(await manager.logs(requireTaskId(args), args.maxBytes));
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
  if (!Object.hasOwn(PROVIDERS, provider)) {
    throw new Error(`provider must be one of: ${Object.keys(PROVIDERS).join(", ")}`);
  }

  const mode = input.mode;
  if (!["research", "review", "implement", "command"].includes(mode)) {
    throw new Error("mode must be one of: research, review, implement, command");
  }
  if (!PROVIDERS[provider].modes.includes(mode)) {
    throw new Error(`${provider} does not support mode: ${mode}`);
  }

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
  if (input.effort !== undefined && (provider !== "claude" || !PROVIDERS.claude.effort.includes(input.effort))) {
    throw new Error(`effort is only supported for claude and must be one of: ${PROVIDERS.claude.effort.join(", ")}`);
  }
  if (input.thinking !== undefined) {
    const allowed = PROVIDERS[provider].thinking;
    if (!allowed?.includes(input.thinking)) {
      throw new Error(`thinking is not supported for ${provider}`);
    }
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
  const timeout = String(task.timeoutSeconds);
  const prompt = renderTaskPrompt(task);
  const bins = options.providerBins ?? {};

  switch (task.provider) {
    case "claude":
      return buildClaudeCommand(task, prompt, timeout, bins);
    case "cursor":
      return {
        command: bins.cursor ?? process.env.CURSOR_AGENT_BIN ?? "cursor-agent",
        args: [
          "-p",
          "--output-format", "json",
          "--workspace", task.cwd,
          ...cursorModeFlags(task.mode),
          ...(task.model ? ["--model", task.model] : []),
          "--trust",
          "--",
          prompt
        ],
        cwd: task.cwd,
        timeoutSeconds: task.timeoutSeconds,
        task
      };
    case "kimi":
      return {
        command: bins.kimi ?? process.env.PI_BIN ?? "pi",
        args: [
          "-p",
          "--no-session",
          "--no-context-files",
          "--tools", kimiTools(task.mode),
          ...(task.model ? ["--model", task.model] : []),
          ...(task.thinking ? ["--thinking", task.thinking] : []),
          prompt
        ],
        cwd: task.cwd,
        timeoutSeconds: task.timeoutSeconds,
        task
      };
    case "codex":
      return {
        command: bins.codex ?? process.env.CODEX_BIN ?? "codex",
        args: [
          "exec",
          "--cd", task.cwd,
          "--json",
          "--sandbox", codexSandbox(task.mode),
          ...(task.model ? ["--model", task.model] : []),
          ...(task.thinking ? ["--config", `model_reasoning_effort="${task.thinking}"`] : []),
          prompt
        ],
        cwd: task.cwd,
        timeoutSeconds: task.timeoutSeconds,
        task
      };
    default:
      throw new Error(`Unknown provider: ${task.provider}`);
  }
}

function buildClaudeCommand(task, prompt, timeout, bins = {}) {
  const claudePBin = bins.claudeP ?? process.env.CLAUDE_P_BIN;
  const nativeClaudeBin = bins.claude ?? process.env.CLAUDE_BIN;
  if (claudePBin || !nativeClaudeBin) {
    return {
      command: claudePBin ?? "claude-p",
      args: [
        "--cwd", task.cwd,
        "--timeout", timeout,
        "--output-format", "json",
        ...claudeModeFlags(task.mode),
        ...(task.model ? ["--model", task.model] : []),
        ...(task.effort ? ["--effort", task.effort] : []),
        "--",
        prompt
      ],
      cwd: task.cwd,
      timeoutSeconds: task.timeoutSeconds,
      task
    };
  }

  return {
    command: nativeClaudeBin,
    args: [
      "-p",
      "--output-format", "json",
      ...claudeModeFlags(task.mode),
      ...(task.model ? ["--model", task.model] : []),
      ...(task.effort ? ["--effort", task.effort] : []),
      "--",
      prompt
    ],
    cwd: task.cwd,
    timeoutSeconds: task.timeoutSeconds,
    task
  };
}

function claudeModeFlags(mode) {
  if (mode === "review" || mode === "research") {
    return ["--permission-mode", "dontAsk", "--allowedTools", "Read,Grep,Glob", "--disallowedTools", "Bash,Edit,Write"];
  }
  if (mode === "command") {
    return ["--permission-mode", "default", "--allowedTools", "Read,Grep,Glob,Bash", "--disallowedTools", "Edit,Write"];
  }
  return ["--permission-mode", "default"];
}

function cursorModeFlags(mode) {
  if (mode === "review" || mode === "research") {
    return ["--mode", "ask"];
  }
  return [];
}

function kimiTools(mode) {
  if (mode === "implement") {
    return "read,bash,edit,write,grep,find,ls";
  }
  if (mode === "command") {
    return "read,bash,grep,find,ls";
  }
  return "read,grep,find,ls";
}

function codexSandbox(mode) {
  if (mode === "review" || mode === "research") {
    return "read-only";
  }
  return "workspace-write";
}

function renderTaskPrompt(task) {
  const title = task.title ? `Title: ${task.title}\n` : "";
  return [
    title,
    `Mode: ${task.mode}`,
    `Provider: ${task.provider}`,
    `Instruction: ${MODE_DESCRIPTIONS[task.mode]}`,
    "",
    task.prompt,
    "",
    "Return a concise final report with: summary, changed files if any, evidence, risks, and next steps."
  ].join("\n");
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
      env: buildProviderEnv(record.provider),
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
      this.finish(record.taskId, { status: "failed", error: error.message }).catch((finishError) => this.logPersistError(record.taskId, finishError));
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
      this.finish(record.taskId, { status, error, exitCode, signal }).catch((finishError) => this.logPersistError(record.taskId, finishError));
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

  async logs(taskId, maxBytes = MAX_LOG_BYTES) {
    const task = this.requireTask(taskId);
    const [stdout, stderr] = await Promise.all([
      readCappedFile(path.join(task.taskDir, "stdout.log"), normalizeMaxBytes(maxBytes)),
      readCappedFile(path.join(task.taskDir, "stderr.log"), normalizeMaxBytes(maxBytes))
    ]);
    return {
      taskId,
      status: task.status,
      stdout: stdout.text,
      stderr: stderr.text,
      stdoutTruncated: stdout.truncated,
      stderrTruncated: stderr.truncated
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

  publicTask(task) {
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
      completedAt: task.completedAt
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

export function buildProviderEnv(provider) {
  if (provider === "claude") {
    const env = { ...process.env };
    delete env.ANTHROPIC_BASE_URL;
    return env;
  }

  const env = {};
  const names = [
    "PATH",
    "HOME",
    "TMPDIR",
    "TERM",
    "COLORTERM",
    "USER",
    "LOGNAME",
    "SHELL",
    "LANG",
    "LC_ALL",
    "CLAUDE_CONFIG_DIR",
    "CLAUDE_BIN",
    "CLAUDE_P_BIN",
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_OAUTH_TOKEN",
    "ANTHROPIC_BASE_URL",
    "CURSOR_AGENT_BIN",
    "CURSOR_API_KEY",
    "PI_BIN",
    "PI_CODING_AGENT_DIR",
    "PI_CODING_AGENT_SESSION_DIR",
    "KIMI_API_KEY",
    "FIREWORKS_API_KEY",
    "GEMINI_API_KEY",
    "OPENROUTER_API_KEY",
    "TOGETHER_API_KEY",
    "OPENAI_BASE_URL",
    "CODEX_BIN",
    "CODEX_HOME",
    "OPENAI_API_KEY",
    "AGENT_BRIDGE_ALLOWED_ROOT",
    "AGENT_BRIDGE_STATE_DIR"
  ];
  for (const name of names) {
    if (process.env[name] !== undefined) {
      env[name] = process.env[name];
    }
  }
  return env;
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

    const child = spawn(command, args, {
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
