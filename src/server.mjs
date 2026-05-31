#!/usr/bin/env node
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import readline from "node:readline";
import { fileURLToPath } from "node:url";

const PROTOCOL_VERSION = "2024-11-05";
const DEFAULT_TIMEOUT_SECONDS = 120;
const MAX_TIMEOUT_SECONDS = 1800;
const MIN_TIMEOUT_SECONDS = 1;
const MAX_BUFFER_BYTES = 1024 * 1024;
const MAX_PROMPT_BYTES = 100 * 1024;
const TRUNCATION_SENTINEL = `[truncated after ${MAX_BUFFER_BYTES} bytes]`;
const STARTUP_CWD = process.cwd();
const activeChildren = new Set();

const TOOL_NAMES = ["ask_claude", "ask_kimi", "ask_cursor", "dispatch_claude", "dispatch_cursor"];
const READ_ONLY_TOOLS = new Set(["ask_claude", "ask_kimi", "ask_cursor"]);
const DISPATCH_TOOLS = new Set(["dispatch_claude", "dispatch_cursor"]);
const SAFE_PERMISSION_MODES = new Set(["dontAsk", "default"]);
const READ_ONLY_FORBIDDEN_FIELDS = new Set(["permissionMode", "allowedTools", "disallowedTools"]);
const COMMON_FIELDS = new Set(["prompt", "cwd", "timeoutSeconds", "dryRun"]);
const TOOL_FIELDS = {
  ask_claude: new Set([...COMMON_FIELDS]),
  ask_kimi: new Set([...COMMON_FIELDS, "contextFiles"]),
  ask_cursor: new Set([...COMMON_FIELDS]),
  dispatch_claude: new Set([...COMMON_FIELDS, "permissionMode", "allowedTools", "disallowedTools"]),
  dispatch_cursor: new Set([...COMMON_FIELDS, "permissionMode", "model"])
};

const toolDefinitions = [
  {
    name: "ask_claude",
    description: "Ask Claude Code for a read-only second opinion via claude-p.",
    inputSchema: baseSchema()
  },
  {
    name: "ask_kimi",
    description: "Ask Kimi through the existing Pi/kimi-review wrapper. Supports contextFiles under cwd.",
    inputSchema: baseSchema({
      contextFiles: {
        type: "array",
        items: { type: "string" },
        description: "Relative file paths under cwd to pass to the Kimi wrapper."
      }
    })
  },
  {
    name: "ask_cursor",
    description: "Ask Cursor Agent for a read-only second opinion in ask mode.",
    inputSchema: baseSchema()
  },
  {
    name: "dispatch_claude",
    description: "Dispatch bounded Claude Code work via claude-p with explicit capability options.",
    inputSchema: baseSchema({
      permissionMode: { type: "string", enum: ["dontAsk", "default"] },
      allowedTools: { type: "array", items: { type: "string" } },
      disallowedTools: { type: "array", items: { type: "string" } }
    })
  },
  {
    name: "dispatch_cursor",
    description: "Dispatch bounded Cursor Agent work via cursor-agent.",
    inputSchema: baseSchema({
      permissionMode: { type: "string", enum: ["dontAsk", "default"] },
      model: { type: "string" }
    })
  }
];

function baseSchema(extraProperties = {}) {
  return {
    type: "object",
    additionalProperties: false,
    required: ["prompt"],
    properties: {
      prompt: { type: "string", description: "Prompt to send to the provider. Maximum 100 KiB UTF-8." },
      cwd: { type: "string", description: "Workspace directory. Must stay under the allowed root." },
      timeoutSeconds: { type: "number", description: "Timeout in seconds, clamped to 1-1800." },
      dryRun: { type: "boolean", description: "Return the command that would run without spawning it." },
      ...extraProperties
    }
  };
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
    const command = await buildCommand(name, args);
    if (args.dryRun) {
      return toolText(`Dry run: ${shellDisplay(command.command, command.args)}`);
    }

    logProvider("start", name, command);
    const result = await runCommand(command.command, command.args, {
      cwd: command.cwd,
      timeoutMs: command.timeoutSeconds * 1000,
      env: buildProviderEnv(),
      maxBufferBytes: MAX_BUFFER_BYTES
    });
    logProvider(result.ok ? "finish" : "failure", name, command);

    if (!result.ok) {
      const detail = [result.error, result.stderr, result.stdout].filter(Boolean).join("\n");
      return toolError(detail || `${name} failed`);
    }

    return toolText(result.stdout || result.stderr || "");
  } catch (error) {
    return toolError(error instanceof Error ? error.message : String(error));
  }
}

export async function buildCommand(toolName, input = {}, options = {}) {
  const validated = await validateToolArguments(toolName, input, options);
  const timeout = String(validated.timeoutSeconds);

  switch (toolName) {
    case "ask_claude":
      return {
        command: process.env.CLAUDE_P_BIN || "claude-p",
        args: [
          "--cwd", validated.cwd,
          "--timeout", timeout,
          "--output-format", "json",
          "--permission-mode", "dontAsk",
          "--allowedTools", "Read,Grep,Glob",
          "--disallowedTools", "Bash,Edit,Write",
          "--",
          validated.prompt
        ],
        cwd: validated.cwd,
        timeoutSeconds: validated.timeoutSeconds
      };
    case "dispatch_claude":
      return {
        command: process.env.CLAUDE_P_BIN || "claude-p",
        args: [
          "--cwd", validated.cwd,
          "--timeout", timeout,
          "--output-format", "json",
          "--permission-mode", validated.permissionMode ?? "default",
          ...(validated.allowedTools?.length ? ["--allowedTools", ...validated.allowedTools] : []),
          ...(validated.disallowedTools?.length ? ["--disallowedTools", ...validated.disallowedTools] : []),
          "--",
          validated.prompt
        ],
        cwd: validated.cwd,
        timeoutSeconds: validated.timeoutSeconds
      };
    case "ask_kimi":
      return {
        command: process.env.KIMI_WRAPPER_PATH || path.join(os.homedir(), ".claude/skills/kimi-review/kimi.sh"),
        args: ["consult", validated.prompt, ...(validated.contextFiles ?? [])],
        cwd: validated.cwd,
        timeoutSeconds: validated.timeoutSeconds
      };
    case "ask_cursor":
      return {
        command: process.env.CURSOR_AGENT_BIN || "cursor-agent",
        args: [
          "-p",
          "--mode", "ask",
          "--output-format", "json",
          "--workspace", validated.cwd,
          "--trust",
          "--",
          validated.prompt
        ],
        cwd: validated.cwd,
        timeoutSeconds: validated.timeoutSeconds
      };
    case "dispatch_cursor":
      return {
        command: process.env.CURSOR_AGENT_BIN || "cursor-agent",
        args: [
          "-p",
          "--output-format", "json",
          "--workspace", validated.cwd,
          ...(validated.model ? ["--model", validated.model] : []),
          "--trust",
          "--",
          validated.prompt
        ],
        cwd: validated.cwd,
        timeoutSeconds: validated.timeoutSeconds
      };
    default:
      throw new Error(`Unknown tool: ${toolName}`);
  }
}

export async function validateToolArguments(toolName, input = {}, options = {}) {
  if (!TOOL_NAMES.includes(toolName)) {
    throw new Error(`Unknown tool: ${toolName}`);
  }

  if (READ_ONLY_TOOLS.has(toolName)) {
    for (const field of READ_ONLY_FORBIDDEN_FIELDS) {
      if (Object.hasOwn(input, field)) {
        throw new Error(`${field} is not allowed for read-only tools`);
      }
    }
  }

  if (input.contextFiles && toolName !== "ask_kimi") {
    throw new Error("contextFiles are only supported by ask_kimi");
  }

  const allowedFields = TOOL_FIELDS[toolName];
  for (const field of Object.keys(input)) {
    if (!allowedFields.has(field)) {
      throw new Error(`Unknown argument for ${toolName}: ${field}`);
    }
  }

  const prompt = input.prompt;
  if (typeof prompt !== "string" || prompt.length === 0) {
    throw new Error("prompt is required");
  }
  if (Buffer.byteLength(prompt, "utf8") > MAX_PROMPT_BYTES) {
    throw new Error(`prompt exceeds ${MAX_PROMPT_BYTES} bytes`);
  }

  if (input.permissionMode && !SAFE_PERMISSION_MODES.has(input.permissionMode)) {
    throw new Error("permissionMode must be one of: dontAsk, default");
  }

  const cwd = await resolveSafeCwd(input.cwd, options);
  const contextFiles = await resolveContextFiles(input.contextFiles, cwd);

  return {
    prompt,
    cwd,
    timeoutSeconds: clampTimeoutSeconds(input.timeoutSeconds),
    dryRun: Boolean(input.dryRun),
    contextFiles,
    permissionMode: input.permissionMode,
    allowedTools: normalizeStringArray(input.allowedTools, "allowedTools"),
    disallowedTools: normalizeStringArray(input.disallowedTools, "disallowedTools"),
    model: input.model
  };
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

async function resolveContextFiles(contextFiles, cwd) {
  if (contextFiles === undefined) {
    return undefined;
  }
  if (!Array.isArray(contextFiles)) {
    throw new Error("contextFiles must be an array");
  }

  const resolved = [];
  for (const file of contextFiles) {
    if (typeof file !== "string" || path.isAbsolute(file) || hasDotDotSegment(file)) {
      throw new Error("contextFiles must be relative paths under cwd");
    }

    const candidate = path.resolve(cwd, file);
    const real = await fs.realpath(candidate);
    const stat = await fs.stat(real);
    if (!stat.isFile()) {
      throw new Error(`contextFiles entry is not a file: ${file}`);
    }
    if (!isPathInside(real, cwd)) {
      throw new Error(`contextFiles entry resolves outside cwd: ${file}`);
    }
    resolved.push(real);
  }
  return resolved;
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

function normalizeStringArray(value, name) {
  if (value === undefined) {
    return undefined;
  }
  if (!Array.isArray(value) || value.some((entry) => typeof entry !== "string" || entry.length === 0)) {
    throw new Error(`${name} must be an array of non-empty strings`);
  }
  return value;
}

export async function runCommand(command, args, options) {
  const timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_SECONDS * 1000;
  const maxBufferBytes = options.maxBufferBytes ?? MAX_BUFFER_BYTES;

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
  const sentinelBytes = Buffer.byteLength(TRUNCATION_SENTINEL, "utf8");
  const keepBytes = Math.max(0, maxBufferBytes - sentinelBytes);

  if (Buffer.byteLength(current, "utf8") >= maxBufferBytes) {
    const truncated = Buffer.from(current, "utf8").subarray(0, keepBytes).toString("utf8") + TRUNCATION_SENTINEL;
    return { text: truncated, truncated: true };
  }

  const next = current + chunk.toString("utf8");
  if (Buffer.byteLength(next, "utf8") <= maxBufferBytes) {
    return { text: next, truncated: false };
  }

  const truncated = Buffer.from(next, "utf8").subarray(0, keepBytes).toString("utf8") + TRUNCATION_SENTINEL;
  return { text: truncated, truncated: true };
}

export function buildProviderEnv() {
  const env = { ...process.env };
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
    "CLAUDE_P_BIN",
    "KIMI_WRAPPER_PATH",
    "KIMI_MODEL",
    "KIMI_THINKING",
    "CURSOR_AGENT_BIN",
    "CURSOR_API_KEY",
    "AGENT_BRIDGE_ALLOWED_ROOT"
  ];
  for (const name of names) {
    if (process.env[name] !== undefined) {
      env[name] = process.env[name];
    }
  }
  return env;
}

function jsonRpcResult(id, result) {
  return { jsonrpc: "2.0", id, result };
}

function jsonRpcError(id, code, message) {
  return { jsonrpc: "2.0", id, error: { code, message } };
}

function toolText(text) {
  return { content: [{ type: "text", text }], isError: false };
}

function toolError(text) {
  return { content: [{ type: "text", text }], isError: true };
}

function shellDisplay(command, args) {
  return [command, ...args].map(quoteShell).join(" ");
}

function quoteShell(value) {
  if (/^[A-Za-z0-9_/:=.,@+-]+$/.test(value)) {
    return value;
  }
  return `'${value.replaceAll("'", "'\\''")}'`;
}

function logProvider(event, toolName, command) {
  console.error(`[agent-bridge] ${event} tool=${toolName} command=${path.basename(command.command)} cwd=${command.cwd}`);
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
