## 1. Contract And Public API

- [x] 1.1 Validate the OpenSpec change before implementation.
- [x] 1.2 Update tool schemas so lifecycle inputs require `agentId` and no longer mention `taskId`.
- [x] 1.3 Update tool dispatch so lifecycle tools read only `agentId` and reject `taskId`.
- [x] 1.4 Update lifecycle response rendering so `agent_spawn`, `agent_list`, `agent_status`, `agent_wait`, `agent_logs`, `agent_transcript`, `agent_observe`, `agent_result`, `agent_stop`, and `agent_remove` use `agentId` and do not include `taskId`.
- [x] 1.5 Rename `agent_observe` nested `task` payload to `agent` if it remains in the response.
- [x] 1.6 Update review packets so identifier fields and recommended action text use `agentId`/`agent_*`.
- [x] 1.7 Update next-action metadata so every tool-call argument uses `agentId`.
- [x] 1.8 Update public tool descriptions, output schemas, public error messages, and action/reason strings so they do not advertise `taskId` or public `task_*` tools.
- [x] 1.9 Generate new lifecycle IDs with the `agent_` prefix.
- [x] 1.10 Serialize new registry records with `agentId`; old registries using `taskId` fail with the existing clear registry parse/startup diagnostic.

## 2. Configuration Simplification

- [x] 2.1 Align `doctor_state` with the task manager default `~/.agent-bridge-mcp/state`.
- [x] 2.2 Update state-dir recommendations and docs so `AGENT_BRIDGE_STATE_DIR` is an optional override, not required for minimal setup.

## 3. Guidance And Documentation

- [x] 3.1 Update initialization instructions, prompts, and guidance resources to remove public `task_*`/`taskId` workflow language.
- [x] 3.2 Update README examples and migration notes for the breaking `taskId` to `agentId` rename.
- [x] 3.3 Update affected OpenSpec deltas and baseline specs where durable requirements still name public `task_*` tools or `taskId`.

## 4. Tests And Verification

- [x] 4.1 Update server protocol tests for `agentId` input schemas, output schemas, and tool descriptions.
- [x] 4.2 Update stdio lifecycle tests to use and assert `agentId`, `agent_` prefixes, and `taskId` rejection across lifecycle inputs.
- [x] 4.3 Add response-level assertions that public JSON from lifecycle tools, review packets, nested observation payloads, and nextActions contain no `taskId` keys.
- [x] 4.4 Add or update default-state-dir coverage for `doctor` using an isolated temp `HOME`/environment so the test does not touch real user state.
- [x] 4.5 Run `cargo fmt --check`.
- [x] 4.6 Run focused Rust tests covering schemas, lifecycle, guidance, and doctor state.
- [x] 4.7 Run `cargo test`.
- [x] 4.8 Run `openspec validate simplify-agent-api-config --strict`.
- [x] 4.9 Build the release binary, install it to `~/.local/bin/agent-bridge-mcp`, compare release and installed binaries, and smoke the installed binary.
