import test from "node:test";
import assert from "node:assert/strict";
import {
  buildProviderCommand,
  buildProviderEnv,
  buildProviderSmokeCommand,
  buildProviderVersionCommand,
  getProviderCapabilities,
  providerNames,
  providerTaskModes,
  validateProviderTaskOptions
} from "../src/provider-registry.mjs";
import { handleRequest } from "../src/server.mjs";

const cwd = process.cwd();

test("provider registry exposes public provider capabilities", () => {
  const capabilities = getProviderCapabilities();

  assert.deepEqual(providerNames(), ["claude", "cursor", "kimi", "codex"]);
  assert.deepEqual(providerTaskModes(), ["research", "review", "implement", "command"]);
  assert.deepEqual(Object.keys(capabilities), ["claude", "cursor", "kimi", "codex"]);
  assert.deepEqual(capabilities.claude.effort, ["low", "medium", "high", "xhigh", "max"]);
  assert.deepEqual(capabilities.kimi.thinking, ["off", "minimal", "low", "medium", "high", "xhigh"]);
  assert.deepEqual(capabilities.codex.thinking, ["low", "medium", "high", "xhigh"]);
  assert.equal(capabilities.cursor.supportsWorktreeIsolation, true);
});

test("provider registry validates provider-specific task options", () => {
  assert.doesNotThrow(() => validateProviderTaskOptions({ provider: "claude", mode: "review", effort: "high" }));
  assert.doesNotThrow(() => validateProviderTaskOptions({ provider: "kimi", mode: "review", thinking: "xhigh" }));
  assert.doesNotThrow(() => validateProviderTaskOptions({ provider: "codex", mode: "review", thinking: "medium" }));

  assert.throws(
    () => validateProviderTaskOptions({ provider: "cursor", mode: "command" }),
    /cursor does not support mode/
  );
  assert.throws(
    () => validateProviderTaskOptions({ provider: "codex", mode: "review", effort: "high" }),
    /effort is only supported for claude/
  );
  assert.throws(
    () => validateProviderTaskOptions({ provider: "cursor", mode: "review", thinking: "high" }),
    /thinking is not supported for cursor/
  );
});

test("provider registry builds stable provider command descriptors", () => {
  const claude = buildProviderCommand(
    { provider: "claude", mode: "review", prompt: "Review", cwd, timeoutSeconds: 10, effort: "high" },
    { providerBins: { claudeP: "/custom/claude-p" } }
  );
  assert.equal(claude.command, "/bin/zsh");
  assert.deepEqual(claude.args.slice(0, 5), [
    "-lc",
    "source ~/.zshenv 2>/dev/null || true; source ~/.zprofile 2>/dev/null || true; source ~/.zshrc 2>/dev/null || true; exec \"$@\"",
    "agent-bridge-provider",
    "/custom/claude-p",
    "--cwd"
  ]);
  assert.ok(claude.args.includes("--effort"));
  assert.ok(claude.args.includes("high"));

  const cursor = buildProviderCommand({ provider: "cursor", mode: "implement", prompt: "Implement", cwd, timeoutSeconds: 20, model: "gpt-5" }, { providerBins: { cursor: "cursor-ok" } });
  assert.equal(cursor.command, "cursor-ok");
  assert.ok(cursor.args.includes("--model"));
  assert.ok(cursor.args.includes("gpt-5"));
  assert.ok(cursor.args.includes("--trust"));

  const kimi = buildProviderCommand({ provider: "kimi", mode: "command", prompt: "Run", cwd, timeoutSeconds: 30, thinking: "low" }, { providerBins: { kimi: "kimi-ok" } });
  assert.equal(kimi.command, "kimi-ok");
  assert.ok(kimi.args.includes("--tools"));
  assert.ok(kimi.args.includes("read,bash,grep,find,ls"));
  assert.ok(kimi.args.includes("--thinking"));

  const codex = buildProviderCommand({ provider: "codex", mode: "review", prompt: "Review", cwd, timeoutSeconds: 40, thinking: "medium" }, { providerBins: { codex: "codex-ok" } });
  assert.equal(codex.command, "codex-ok");
  assert.deepEqual(codex.args.slice(0, 3), ["exec", "--cd", cwd]);
  assert.ok(codex.args.includes("--json"));
  assert.ok(codex.args.includes("shell_environment_policy.inherit=\"all\""));
});

test("provider registry owns provider environment policy", () => {
  const previousTerm = process.env.TERM;
  const previousAnthropic = process.env.ANTHROPIC_API_KEY;
  const previousAnthropicBaseUrl = process.env.ANTHROPIC_BASE_URL;
  process.env.TERM = "xterm-256color";
  process.env.ANTHROPIC_API_KEY = "test-anthropic-key";
  process.env.ANTHROPIC_BASE_URL = "http://127.0.0.1:8787";
  try {
    const claudeEnv = buildProviderEnv("claude");
    const codexEnv = buildProviderEnv("codex");
    assert.equal(claudeEnv.TERM, "xterm-256color");
    assert.equal(claudeEnv.ANTHROPIC_API_KEY, "test-anthropic-key");
    assert.equal(claudeEnv.ANTHROPIC_BASE_URL, undefined);
    assert.equal(codexEnv.ANTHROPIC_BASE_URL, "http://127.0.0.1:8787");
  } finally {
    if (previousTerm === undefined) delete process.env.TERM;
    else process.env.TERM = previousTerm;
    if (previousAnthropic === undefined) delete process.env.ANTHROPIC_API_KEY;
    else process.env.ANTHROPIC_API_KEY = previousAnthropic;
    if (previousAnthropicBaseUrl === undefined) delete process.env.ANTHROPIC_BASE_URL;
    else process.env.ANTHROPIC_BASE_URL = previousAnthropicBaseUrl;
  }
});

test("provider registry uses matching binary resolution for version and smoke commands", () => {
  const bins = {
    claudeP: "claude-ok",
    cursor: "cursor-ok",
    kimi: "kimi-ok",
    codex: "codex-ok"
  };

  for (const provider of providerNames()) {
    const version = buildProviderVersionCommand(provider, { providerBins: bins });
    const smoke = buildProviderSmokeCommand(provider, { cwd, timeoutSeconds: 3, providerBins: bins });
    assert.equal(smoke.command === "/bin/zsh" ? smoke.args[3] : smoke.command, version.command);
    assert.deepEqual(version.args, ["--version"]);
    assert.ok(smoke.args.some((arg) => arg.includes("AGENT_BRIDGE_PROVIDER_SMOKE_OK")));
  }
});

test("tools/list provider and mode enums stay aligned with the provider registry", async () => {
  const response = await handleRequest({ jsonrpc: "2.0", id: 1, method: "tools/list", params: {} });
  const taskPreview = response.result.tools.find((tool) => tool.name === "task_preview");
  const taskSpawn = response.result.tools.find((tool) => tool.name === "task_spawn");

  assert.deepEqual(taskPreview.inputSchema.properties.provider.enum, providerNames());
  assert.deepEqual(taskSpawn.inputSchema.properties.provider.enum, providerNames());
  assert.deepEqual(taskPreview.inputSchema.properties.mode.enum, providerTaskModes());
  assert.deepEqual(taskSpawn.inputSchema.properties.mode.enum, providerTaskModes());
});
