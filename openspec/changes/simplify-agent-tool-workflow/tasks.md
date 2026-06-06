## 1. Guidance And Docs

- [x] 1.1 Update initialization instructions to lead with the compact primary workflow and name lower-level tools as focused diagnostics.
- [x] 1.2 Rewrite `CALLER_WORKFLOW_RESOURCE` into primary workflow and diagnostic/recovery sections while preserving manual recovery and raw-evidence workflows.
- [x] 1.3 Update MCP prompts and other guidance resources to teach the compact path while preserving Codex denial, stalled-task recovery, and raw-evidence workflows.
- [x] 1.4 Update README recommended workflow and tool summary so common usage does not present `agent_preview`, `agent_list`, status, wait, logs, and transcript as required default steps.

## 2. Tool Surface Wording

- [x] 2.1 Update public tool descriptions in `tools/list` to distinguish primary lifecycle tools from focused readiness, preview, status, finality, log, and transcript tools.
- [x] 2.2 Preserve every current public MCP tool name, strict schema, and response compatibility; do not add or remove tools.

## 3. Tests

- [x] 3.1 Add or update protocol tests for initialization guidance, prompt/resource guidance, and public tool descriptions using ordered key phrases or section assertions rather than full paragraph equality.
- [x] 3.2 Add schema-compatibility assertions proving all current public tools keep their input schemas, required fields, and strict `additionalProperties` behavior while descriptions change.
- [x] 3.3 Add or update stdio fixture tests proving the public tool list contains the current `agent_*` surface and no legacy `task_*` lifecycle tools.
- [x] 3.4 Confirm existing next-action tests prove running agents rank `agent_observe` first and final uninspected agents rank `agent_result` first; add regression assertions only if coverage is missing.

## 4. Validation

- [x] 4.1 Run formatting checks.
- [x] 4.2 Run focused protocol and stdio tests affected by the guidance/tool-surface changes.
- [x] 4.3 Run full cargo tests and clippy.
- [x] 4.4 Run `openspec validate simplify-agent-tool-workflow`.
- [x] 4.5 Build the release binary, update the installed `~/.local/bin/agent-bridge-mcp`, compare release and installed binaries, and smoke the installed binary.
- [x] 4.6 Note archive follow-up: merge the deltas into base specs, especially the `agent_observe` primary running-agent action requirement.
