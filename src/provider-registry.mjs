const PROVIDER_SMOKE_PROMPT = "Reply with exactly: AGENT_BRIDGE_PROVIDER_SMOKE_OK";

const TASK_MODES = ["research", "review", "implement", "command"];

const MODE_DESCRIPTIONS = {
  research: "Read and analyze. Do not edit files.",
  review: "Review the requested code or plan. Do not edit files.",
  implement: "Make the requested code changes, keep scope tight, and report verification evidence.",
  command: "Run the requested bounded command-oriented task and report evidence."
};

const PROVIDERS = {
  claude: {
    modes: ["research", "review", "implement", "command"],
    supportsReply: false,
    supportsResume: false,
    supportsWorktreeIsolation: true,
    effort: ["low", "medium", "high", "xhigh", "max"],
    command: buildClaudeCommand,
    env: buildClaudeEnv,
    resolveCommand: resolveClaudeCommand
  },
  cursor: {
    modes: ["research", "review", "implement"],
    supportsReply: false,
    supportsResume: false,
    supportsWorktreeIsolation: true,
    command: buildCursorCommand,
    env: buildSharedProviderEnv,
    resolveCommand: (bins = {}) => bins.cursor ?? process.env.CURSOR_AGENT_BIN ?? "cursor-agent"
  },
  kimi: {
    modes: ["research", "review", "implement", "command"],
    supportsReply: false,
    supportsResume: false,
    supportsWorktreeIsolation: true,
    thinking: ["off", "minimal", "low", "medium", "high", "xhigh"],
    command: buildKimiCommand,
    env: buildSharedProviderEnv,
    resolveCommand: (bins = {}) => bins.kimi ?? process.env.PI_BIN ?? "pi"
  },
  codex: {
    modes: ["research", "review", "implement", "command"],
    supportsReply: false,
    supportsResume: false,
    supportsWorktreeIsolation: true,
    thinking: ["low", "medium", "high", "xhigh"],
    command: buildCodexCommand,
    env: buildSharedProviderEnv,
    resolveCommand: (bins = {}) => bins.codex ?? process.env.CODEX_BIN ?? "codex"
  }
};

export function providerNames() {
  return Object.keys(PROVIDERS);
}

export function providerTaskModes() {
  return [...TASK_MODES];
}

export function getProviderCapabilities() {
  const capabilities = {};
  for (const [name, provider] of Object.entries(PROVIDERS)) {
    const { command, env, resolveCommand, ...publicCapability } = provider;
    capabilities[name] = clone(publicCapability);
  }
  return capabilities;
}

export function hasProvider(name) {
  return Object.hasOwn(PROVIDERS, name);
}

export function validateProviderTaskOptions(task) {
  const provider = requireProvider(task.provider);

  if (!TASK_MODES.includes(task.mode)) {
    throw new Error(`mode must be one of: ${TASK_MODES.join(", ")}`);
  }
  if (!provider.modes.includes(task.mode)) {
    throw new Error(`${task.provider} does not support mode: ${task.mode}`);
  }
  if (task.effort !== undefined && (task.provider !== "claude" || !PROVIDERS.claude.effort.includes(task.effort))) {
    throw new Error(`effort is only supported for claude and must be one of: ${PROVIDERS.claude.effort.join(", ")}`);
  }
  if (task.thinking !== undefined) {
    const allowed = provider.thinking;
    if (!allowed?.includes(task.thinking)) {
      throw new Error(`thinking is not supported for ${task.provider}`);
    }
  }
}

export function buildProviderCommand(task, options = {}) {
  validateProviderTaskOptions(task);
  return requireProvider(task.provider).command(task, options.providerBins ?? {});
}

export function buildProviderEnv(providerName) {
  if (providerName === undefined) {
    return buildSharedProviderEnv();
  }
  return requireProvider(providerName).env();
}

export function buildProviderVersionCommand(providerName, options = {}) {
  const command = requireProvider(providerName).resolveCommand(options.providerBins ?? {});
  return {
    command,
    args: ["--version"],
    env: buildProviderEnv(providerName)
  };
}

export function buildProviderSmokeCommand(providerName, options = {}) {
  return buildProviderCommand({
    provider: providerName,
    mode: "research",
    prompt: PROVIDER_SMOKE_PROMPT,
    cwd: options.cwd,
    timeoutSeconds: options.timeoutSeconds
  }, { providerBins: options.providerBins });
}

function buildClaudeCommand(task, bins = {}) {
  const prompt = renderTaskPrompt(task);
  const timeout = String(task.timeoutSeconds);
  const claudePBin = bins.claudeP ?? process.env.CLAUDE_P_BIN;
  const nativeClaudeBin = bins.claude ?? process.env.CLAUDE_BIN;
  if (claudePBin || !nativeClaudeBin) {
    return maybeWrapWithShellInit({
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
    });
  }

  return maybeWrapWithShellInit({
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
  });
}

function buildCursorCommand(task, bins = {}) {
  const prompt = renderTaskPrompt(task);
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
}

function buildKimiCommand(task, bins = {}) {
  const prompt = renderTaskPrompt(task);
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
}

function buildCodexCommand(task, bins = {}) {
  const prompt = renderTaskPrompt(task);
  return {
    command: bins.codex ?? process.env.CODEX_BIN ?? "codex",
    args: [
      "exec",
      "--cd", task.cwd,
      "--json",
      "--sandbox", codexSandbox(task.mode),
      ...codexEnvironmentConfigArgs(),
      ...(task.model ? ["--model", task.model] : []),
      ...(task.thinking ? ["--config", `model_reasoning_effort="${task.thinking}"`] : []),
      prompt
    ],
    cwd: task.cwd,
    timeoutSeconds: task.timeoutSeconds,
    task
  };
}

function resolveClaudeCommand(bins = {}) {
  const claudePBin = bins.claudeP ?? process.env.CLAUDE_P_BIN;
  const nativeClaudeBin = bins.claude ?? process.env.CLAUDE_BIN;
  return claudePBin || !nativeClaudeBin ? (claudePBin ?? "claude-p") : nativeClaudeBin;
}

function maybeWrapWithShellInit(command) {
  return {
    ...command,
    command: "/bin/zsh",
    args: [
      "-lc",
      "source ~/.zshenv 2>/dev/null || true; source ~/.zprofile 2>/dev/null || true; source ~/.zshrc 2>/dev/null || true; exec \"$@\"",
      "agent-bridge-provider",
      command.command,
      ...command.args
    ]
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

function codexEnvironmentConfigArgs() {
  return ["--config", "shell_environment_policy.inherit=\"all\""];
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

function buildClaudeEnv() {
  const env = pickEnv([
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
    "LC_CTYPE",
    "XDG_CONFIG_DIRS",
    "XDG_DATA_DIRS",
    "NIX_PROFILES",
    "NIX_SSL_CERT_FILE",
    "NIX_USER_PROFILE_DIR",
    "SSL_CERT_FILE",
    "CLAUDE_CONFIG_DIR",
    "CLAUDE_BIN",
    "CLAUDE_P_BIN",
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_OAUTH_TOKEN",
    "AGENT_BRIDGE_ALLOWED_ROOT",
    "AGENT_BRIDGE_STATE_DIR"
  ]);
  // Codex Desktop can inject an Anthropic proxy endpoint that breaks Claude Code auth.
  delete env.ANTHROPIC_BASE_URL;
  return env;
}

function buildSharedProviderEnv() {
  return pickEnv([
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
  ]);
}

function pickEnv(names) {
  const env = {};
  for (const name of names) {
    if (process.env[name] !== undefined) {
      env[name] = process.env[name];
    }
  }
  return env;
}

function requireProvider(name) {
  const provider = PROVIDERS[name];
  if (!provider) {
    throw new Error(`provider must be one of: ${providerNames().join(", ")}`);
  }
  return provider;
}

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}
