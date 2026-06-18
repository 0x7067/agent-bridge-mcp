use serde::Deserialize;
use serde_json::{Value, json};

const CALLER_WORKFLOW_URI: &str = "agent-bridge://guidance/caller-workflow";
const SAFETY_URI: &str = "agent-bridge://guidance/safety";
const PROVIDER_CAPABILITIES_URI: &str = "agent-bridge://guidance/provider-capabilities";
const CLAUDE_HOST_LIFECYCLE_URI: &str = "agent-bridge://guidance/claude-host-lifecycle";
const DOGFOOD_WORKFLOWS_URI: &str = "agent-bridge://guidance/dogfood-workflows";
const CODE_EXECUTION_URI: &str = "agent-bridge://guidance/code-execution";

pub const INITIALIZATION_INSTRUCTIONS: &str = r#"Agent Bridge delegates review, research, command, and implementation work to provider agents. When multiple providers are available, prefer a provider different from the calling agent unless the task needs that provider. Provider output is evidence only: the caller still owns project verification before claiming work is done. Eight-tool workflow: doctor for setup/readiness (focus:"providers" readiness-only; smoke:true launch check), agent_spawn (dryRun:true preview; spawned agents get the lean-only final-output contract), agent_observe (until:"final" waits; limit:0 state only; events are the transcript), react to notifications/agent_bridge/agent_completed when a current-session agent finishes, agent_result for evidence (stdout/stderr/diff/transcript on demand), verify locally, then agent_remove after managed worktree inspection. Use agent_list as the attention inbox; filters show older inspected results. Use agent_stop when no longer useful. Tool responses are lean by default; verbosity:"detailed" adds debug metadata. Follow structuredContent and the single next action list when present."#;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromptGetParams {
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceReadParams {
    pub uri: String,
}

pub fn prompt_definitions() -> Vec<Value> {
    vec![
        prompt_definition(
            "agent_bridge_delegate_review",
            "Prepare a read-only provider review task and inspect the result as evidence.",
        ),
        prompt_definition(
            "agent_bridge_delegate_implementation",
            "Prepare an isolated implementation task and keep verification in the main caller.",
        ),
        prompt_definition(
            "agent_bridge_inspect_result",
            "Inspect a finished provider task result, logs, diff, and changed files.",
        ),
        prompt_definition(
            "agent_bridge_recover_stalled_task",
            "Recover from a stalled provider task using bounded waits, logs, stop, and result inspection.",
        ),
        prompt_definition(
            "agent_bridge_claude_host_lifecycle",
            "Operate the Claude host runner lifecycle for sandbox-safe Claude delegation.",
        ),
        prompt_definition(
            "agent_bridge_dogfood_workflows",
            "Run reproducible Agent Bridge dogfood workflows without making live provider execution mandatory.",
        ),
        prompt_definition(
            "agent_bridge_compare_providers",
            "Compare provider behavior with bounded read-only tasks and caller-owned verification.",
        ),
    ]
}

pub fn get_prompt(params: Value) -> Result<Value, String> {
    let params: PromptGetParams = serde_json::from_value(params)
        .map_err(|error| format!("Invalid prompt params: {error}"))?;
    if !params.arguments.is_object() && !params.arguments.is_null() {
        return Err("Invalid prompt params: arguments must be an object".to_string());
    }

    let (description, text) = match params.name.as_str() {
        "agent_bridge_delegate_review" => (
            "Delegate a read-only review task through Agent Bridge.",
            REVIEW_PROMPT,
        ),
        "agent_bridge_delegate_implementation" => (
            "Delegate an isolated implementation task through Agent Bridge.",
            IMPLEMENTATION_PROMPT,
        ),
        "agent_bridge_inspect_result" => (
            "Inspect a completed Agent Bridge task result.",
            INSPECT_RESULT_PROMPT,
        ),
        "agent_bridge_recover_stalled_task" => (
            "Recover from a stalled Agent Bridge task.",
            RECOVER_STALLED_PROMPT,
        ),
        "agent_bridge_claude_host_lifecycle" => (
            "Operate the Claude host runner lifecycle.",
            CLAUDE_HOST_LIFECYCLE_PROMPT,
        ),
        "agent_bridge_dogfood_workflows" => (
            "Run Agent Bridge dogfood delegation workflows.",
            DOGFOOD_WORKFLOWS_PROMPT,
        ),
        "agent_bridge_compare_providers" => (
            "Compare Agent Bridge providers safely.",
            COMPARE_PROVIDERS_PROMPT,
        ),
        _ => return Err(format!("Unknown prompt: {}", params.name)),
    };

    Ok(json!({
        "description": description,
        "messages": [
            {
                "role": "user",
                "content": {
                    "type": "text",
                    "text": text
                }
            }
        ]
    }))
}

pub fn resource_definitions() -> Vec<Value> {
    vec![
        resource_definition(
            CALLER_WORKFLOW_URI,
            "caller-workflow",
            "Agent Bridge caller workflow",
        ),
        resource_definition(
            SAFETY_URI,
            "safety",
            "Agent Bridge delegation safety guidance",
        ),
        resource_definition(
            PROVIDER_CAPABILITIES_URI,
            "provider-capabilities",
            "Agent Bridge provider capability summary",
        ),
        resource_definition(
            CLAUDE_HOST_LIFECYCLE_URI,
            "claude-host-lifecycle",
            "Claude host runner lifecycle guidance",
        ),
        resource_definition(
            DOGFOOD_WORKFLOWS_URI,
            "dogfood-workflows",
            "Reproducible Agent Bridge dogfood workflows",
        ),
        resource_definition(
            CODE_EXECUTION_URI,
            "code-execution",
            "Code-execution-friendly delegation for token-efficient callers",
        ),
    ]
}

pub fn read_resource(params: Value) -> Result<Value, String> {
    let params: ResourceReadParams = serde_json::from_value(params)
        .map_err(|error| format!("Resource not found: invalid params: {error}"))?;

    let text = match params.uri.as_str() {
        CALLER_WORKFLOW_URI => CALLER_WORKFLOW_RESOURCE,
        SAFETY_URI => SAFETY_RESOURCE,
        PROVIDER_CAPABILITIES_URI => PROVIDER_CAPABILITIES_RESOURCE,
        CLAUDE_HOST_LIFECYCLE_URI => CLAUDE_HOST_LIFECYCLE_RESOURCE,
        DOGFOOD_WORKFLOWS_URI => DOGFOOD_WORKFLOWS_RESOURCE,
        CODE_EXECUTION_URI => CODE_EXECUTION_RESOURCE,
        _ => return Err(format!("Resource not found: {}", params.uri)),
    };

    Ok(json!({
        "contents": [
            {
                "uri": params.uri,
                "mimeType": "text/markdown",
                "text": text
            }
        ]
    }))
}

fn prompt_definition(name: &str, description: &str) -> Value {
    json!({
        "name": name,
        "description": description,
        "arguments": []
    })
}

fn resource_definition(uri: &str, name: &str, description: &str) -> Value {
    json!({
        "uri": uri,
        "name": name,
        "description": description,
        "mimeType": "text/markdown"
    })
}

const REVIEW_PROMPT: &str = r#"Use Agent Bridge for a read-only provider review.

Suggested flow:
1. Call doctor only when setup/readiness is uncertain; use focus:"providers" and smoke:true when launchability matters.
2. Use agent_spawn mode "review" or "research" with a bounded prompt; dryRun:true previews launch. The provider gets the lean-only final-output contract.
3. Use agent_observe for progress; until:"final" waits for outcome.
4. Read agent_result.reviewPacket; request sections:["stdout","stderr","diff","transcript"] only for raw evidence.
5. Provider output is evidence; the main caller remains responsible for judging findings.
6. Stop or ignore agents after useful evidence; do not wait for source echo, progress narration, generic checklists, or polish.

Use agent_list for active/recent summaries."#;

const IMPLEMENTATION_PROMPT: &str = r#"Use Agent Bridge for isolated implementation work.

Suggested flow:
1. Call doctor only when setup/readiness is uncertain (focus:"providers").
2. Call agent_spawn mode "implement" with a clear prompt, allowed cwd, and isolation "worktree"; dryRun:true previews launch. Providers get the lean-only final-output contract.
3. Use agent_observe for progress (until:"final" waits; limit:0 checks state).
4. When final, call agent_result, inspect reviewPacket, then request sections:["stdout","stderr","diff","transcript"] and changedFiles as needed.
5. The main caller remains responsible for tests, lint, typecheck, build, or OpenSpec validation before claiming completion.
6. Call agent_remove only after inspecting the managed worktree and choosing cleanup."#;

const INSPECT_RESULT_PROMPT: &str = r#"Inspect an Agent Bridge task result.

Use agent_result for the final payload. Start with reviewPacket, status, changedFiles, and next; request sections:["stdout","stderr","transcript","diff"] only when raw evidence is needed.

Do not treat provider completion as final verification. The main caller remains responsible for checking the original request and running the smallest relevant proof before claiming completion."#;

const RECOVER_STALLED_PROMPT: &str = r#"Recover a stalled Agent Bridge task.

Suggested flow:
1. Call agent_observe with timeout/cursor for new lifecycle or transcript events.
2. If needed, call agent_result sections:["stdout","stderr","transcript"] with cursors to avoid rereading the run.
3. Use agent_observe limit:0 for state, or until:"final" when only finality matters.
4. Stop only when no longer useful, then inspect final agent_result.
5. Decide in the main caller whether to discard, narrow, or continue manually.

Codex "patch rejected", sandbox denial, approval denial, outside of the project, or out-of-workspace writes are prompt-scope or workspace-scope failures. Use bounded agent_observe and final agent_result evidence; inspect cwd, workspace policy, prompt scope, and isolation strategy before retrying. Do not loosen sandbox permissions as a reflex."#;

const CLAUDE_HOST_LIFECYCLE_PROMPT: &str = r#"Operate the Claude host runner lifecycle.

Use this when Claude Code auth needs host resources unavailable to the sandboxed MCP process.

Suggested flow:
1. Start `agent-bridge-mcp claude-host-runner <socket>` outside the sandbox with the MCP server's AGENT_BRIDGE_WORKSPACES.
2. Call doctor to confirm socket, workspace policy, and host-runner status.
3. Use host-runner `ping` or Claude-only doctor focus:"providers" smoke when doctor reports a host-runner problem.
4. On workspace_policy_mismatch, restart the runner after updating AGENT_BRIDGE_WORKSPACES.
5. Stop with SIGTERM or SIGINT so active Claude children are reaped.
6. For stale sockets or AGENT_BRIDGE_CLAUDE_HOST_SOCKET failures, inspect diagnostics and restart; do not silently fall back."#;

const DOGFOOD_WORKFLOWS_PROMPT: &str = r#"Run Agent Bridge dogfood workflows.

Suggested workflows:
1. Read-only review: agent_spawn mode "review"/"research", isolation "none", small prompt, bounded agent_observe, final agent_result.
2. Isolated implementation: agent_spawn mode "implement", isolation "worktree"; inspect reviewPacket, request diff/log sections as needed, then verify in the main caller.
3. Stalled recovery: bounded agent_observe, agent_result stdout/stderr/transcript cursors, agent_stop only if no longer useful. For Codex patch rejected, sandbox denial, approval denial, outside of the project, or out-of-workspace writes, inspect cwd, workspace policy, prompt scope, and isolation strategy before retrying.
4. Provider comparison: run equivalent read-only prompts; pair "bridge"/"bare" only when useful, and reserve "unblocked" for workspace-permission checks. Compare reviewPacket, transcript evidence, profileDiagnostics, and provider prose.
5. Use agent_list for active/recent provider-agent summaries.

Live provider execution remains opt-in and should not be added to default CI."#;

const COMPARE_PROVIDERS_PROMPT: &str = r#"Compare Agent Bridge providers safely.

Suggested flow:
1. Call doctor focus:"providers" only when selected-provider readiness needs verification; smoke only for startup proof.
2. Spawn equivalent read-only review/research tasks with short prompts and bounded timeouts. All profiles use the same lean-only final-output contract; use "bare" for reduced config and "unblocked" only for workspace-permission reach after dryRun review.
3. Use agent_observe for progress and agent_result for final evidence; until:"final" waits for outcome.
4. Compare reviewPacket, transcript/stdout/stderr sections, diagnostics, exit metadata, profileDiagnostics, and provider prose.
5. Keep correctness decisions and project verification in the main caller."#;

const CALLER_WORKFLOW_RESOURCE: &str = r#"# Agent Bridge Caller Workflow

Use Agent Bridge when a separate coding agent can provide useful research, review, command execution, or isolated implementation work.

Primary flow:
1. Call `doctor` when setup, workspace, state, provider, host-runner, client registration, or binary freshness is uncertain. Use `focus: "providers"` for a readiness-only check and `smoke: true` when startup readiness matters.
2. Call `agent_spawn` for the real delegated provider agent. Set `dryRun: true` to preview command, cwd, environment, profile, and isolation without spawning. Every spawned provider receives the same lean-only final-output contract.
3. Call `agent_observe` with a bounded timeout to wait for transcript and lifecycle progress. `until: "final"` blocks to finality, `limit: 0` returns lifecycle state only, and the `events` stream is the agent transcript.
4. Once final, call `agent_result` for `reviewPacket`, `changedFiles`, exit metadata, and the single `next` action list. Request `sections: ["stdout","stderr","diff","transcript"]` to fetch raw evidence on demand.
5. Treat provider output and completion as evidence for the main caller, not as final verification.
6. Call `agent_remove` intentionally after any managed worktree has been inspected. The `next` list marks cleanup as `unsafe` for managed worktree tasks until result inspection is explicit.

Notes:
- When several providers are launchable, prefer one different from the caller's own client unless same-provider behavior is the point or the other providers lack the needed capability.
- Inspect `doctor.clients` for static user-level MCP client config diagnostics. It reads only `~/.codex/config.toml`, `~/.claude.json`, and `~/.cursor/mcp.json`; it does not edit config, run client CLIs, search project-level overrides, or prove startup. Follow `kind: "shell"` recommendations such as `codex mcp list` or `claude mcp list` when you need client-side verification.
- Inspect `doctor.binary` for read-only freshness evidence about the running, installed, and release Agent Bridge binaries. It may recommend shell build/install commands, but it does not build, copy, install, or delete binaries.
- Inspect `doctor.taskExtensionReadiness` only as passive evidence about task-like client metadata observed during `initialize` or request `_meta`. It always reports `serverAdvertisesTasks: false`; protocol-level `tasks/*`, `CreateTaskResult`, listing, cancellation, and notifications remain unavailable until a future implementation change.
- Use `AGENT_BRIDGE_WORKSPACES` for workspace policy. `AGENT_BRIDGE_STATE_DIR` is optional; when omitted, runtime state and doctor diagnostics use `~/.agent-bridge-mcp/state`.
- Tool responses are lean by default (each field once, no GUI chrome). Provider final output is lean-only across launch profiles. Pass `verbosity: "detailed"` on `agent_observe`/`agent_result` for debug metadata.
- Use `agent_list` for bounded attention summaries: active agents first, then finished agents whose result has not yet been inspected.
- Provider agents are not interactive or resumable in v1.

Self-guided clients should read `initialize.instructions`, `structuredContent`, output schemas, and the `next` list when available. Clients that ignore those fields can still follow the primary flow manually.

Protocol-level MCP Tasks are distinct from Agent Bridge agent/task tools. Use `agent_spawn`, `agent_list`, and the stable `agent_*` lifecycle by default; `doctor.taskExtensionReadiness` can report observed client metadata, but protocol task support depends on a future negotiated implementation and remains unavailable here.
"#;

const SAFETY_RESOURCE: &str = r#"# Agent Bridge Safety Guidance

- Keep the main caller responsible for project gates and final claims.
- Run relevant tests, lint, typecheck, build, config validation, or OpenSpec validation before saying work is complete.
- Prefer `research` and `review` modes for read-only second opinions.
- Prefer `implement` with `isolation: "worktree"` so provider edits can be inspected before integration.
- Use `command` mode only for bounded command-oriented work with explicit expected evidence.
- Do not remove a managed worktree until the final result, git status, diff, and changed files have been inspected.
- If a task appears stalled, use bounded `agent_observe` (including `until: "final"` and `limit: 0` for a state check) and `agent_stop` only after deciding the agent is no longer useful.
- Stop or ignore provider agents once they have produced useful evidence; do not spend context waiting for source echo, progress narration, generic checklists, or polish.
- Use `agent_result` `sections: ["transcript"]` for behavior analysis, provider comparison, and final/partial result evidence; it does not replace raw logs or main-thread verification.
- For Codex patch rejected, sandbox denial, approval denial, outside of the project, or out-of-workspace write symptoms, use bounded `agent_observe` and final `agent_result`; inspect cwd, workspace policy, prompt scope, and isolation before retrying.
- Do not loosen Codex sandbox permissions as a reflex or repeat an unchanged request after denial diagnostics.
"#;

const PROVIDER_CAPABILITIES_RESOURCE: &str = r#"# Agent Bridge Provider Capabilities

First-class providers:
- `claude`: local Claude Code ACP command. `CLAUDE_ACP_BIN` defaults to `claude-agent`; `CLAUDE_ACP_ARGS` appends optional arguments.
- `cursor`: local Cursor ACP command. `CURSOR_ACP_BIN` is required; `CURSOR_ACP_ARGS` appends optional arguments.
- `kimi`: local Kimi ACP command. `KIMI_ACP_BIN` defaults to `kimi` with default arg `acp`; `KIMI_ACP_ARGS` appends optional arguments.
- `codex`: local Codex ACP command. `CODEX_ACP_BIN` is required; `CODEX_ACP_ARGS` appends optional arguments. Codex patch rejected, sandbox denial, approval denial, outside of the project, or out-of-workspace write symptoms should be investigated with bounded `agent_observe`, final `agent_result` (including `sections: ["stdout","stderr"]`), `agent_spawn dryRun:true`, cwd, workspace policy, prompt scope, and isolation strategy before retrying.
- `forge`: local Forge ACP command. `FORGE_ACP_BIN` is required; `FORGE_ACP_ARGS` appends optional arguments.
- `antigravity`: local Google Antigravity ACP command. `ANTIGRAVITY_ACP_BIN` is required; `ANTIGRAVITY_ACP_ARGS` appends optional arguments. Version checks prove binary availability only; smoke checks may fail until Antigravity auth is available through the local OS keyring or browser OAuth flow.

Supported modes:
- `research`: read/analyze only.
- `review`: read-only review.
- `implement`: write-capable implementation.
- `command`: bounded command-oriented work.

Use `providers_list` for the authoritative runtime provider summary, including launch profiles and reduced-configuration metadata. Use `doctor` with `focus: "providers"` for availability and startup checks. Do not loosen Codex sandbox permissions as a reflex or repeat an unchanged request after denial diagnostics.

Antigravity `research` and `review` modes pass `--sandbox`, but Agent Bridge does not claim verified read-only filesystem enforcement for Antigravity. Treat those modes as prompt-enforced unless local implementation evidence proves stronger sandbox behavior.

Native-client presentation:
- `providers_list` reports `supportsReply`, `supportsResume`, and `presentationActions` so clients can render supported and unsupported controls without trial-and-error task calls.
- `providers_list` includes a non-blocking `readiness` snapshot. Static discovery starts as `state: "stale"` and `launchable: false`; use `doctor` with `focus: "providers"` to refresh readiness.
- Version-only checks keep `launchable: false` unless a task-path smoke probe succeeds. Smoke-verified providers report `readiness.state: "ready"` and `launchable: true`.
- `reply` and `resume` are unsupported for provider tasks in v1. Clients should render them as unavailable actions with explanatory reasons, not as failed tool calls.
"#;

const CLAUDE_HOST_LIFECYCLE_RESOURCE: &str = r#"# Claude Host Runner Lifecycle

Use `agent-bridge-mcp claude-host-runner <socket>` when Claude provider calls need host access that the sandboxed MCP server does not have, such as macOS Keychain-backed Claude Code auth.

Lifecycle:
1. Start the runner outside the sandbox with the same `AGENT_BRIDGE_WORKSPACES` value as the MCP server.
2. Confirm readiness with `doctor`, then use the host-runner `ping` request or a Claude-only `doctor` `focus: "providers"` smoke for focused follow-up.
3. Configure the MCP server with `AGENT_BRIDGE_CLAUDE_HOST_SOCKET`.
4. Restart the runner after workspace-policy changes; a `workspace_policy_mismatch` diagnostic means the runner and MCP server disagree about `AGENT_BRIDGE_WORKSPACES`.
5. Stop the runner with SIGTERM or SIGINT so it stops accepting new connections and terminates active Claude children.
6. Treat `host_runner_unavailable` as a setup problem to inspect, not as permission to silently fall back to sandboxed Claude execution.

Socket behavior:
- The socket directory must be owner-only.
- A stale socket may be removed only after a connection probe confirms no live runner is listening.
- A live socket causes startup to fail without unlinking it.
"#;

const DOGFOOD_WORKFLOWS_RESOURCE: &str = r#"# Agent Bridge Dogfood Workflows

These workflows are reproducible local operator checks. They intentionally keep live provider execution opt-in and outside default CI.

## read-only review

Use `agent_spawn` with mode `review` or `research`, `isolation: "none"`, a small prompt, and a bounded timeout. Use `agent_observe` as the primary progress path, then inspect `agent_result.reviewPacket` and request `sections: ["transcript","stdout","stderr","diff"]` for raw evidence. Use `agent_observe` with `until: "final"` when simple finality is enough.

## active task list

Use `agent_list` with default arguments as an attention inbox: active provider agents first, then final agents whose result has not yet been inspected. Each record is a lean summary (identity, status, phase, progress, primary `next` action). Use filters when looking up inspected history.

## isolated implementation

Use `agent_spawn` with mode `implement` and `isolation: "worktree"`. After completion, inspect `reviewPacket`, then request `agent_result` `sections: ["diff"]` and `changedFiles`; run the relevant verification in the main caller; call `agent_remove` only after the managed worktree has been reviewed.

## stalled-task recovery

Use bounded `agent_observe` calls. If observation does not produce useful evidence, request `agent_result` `sections: ["stdout","stderr"]` with `stdoutLine`/`stderrLine` cursors and `sections: ["transcript"]`, then `agent_observe` with `limit: 0` for a state check. Call `agent_stop` only when the task is no longer useful, then inspect final `agent_result`.

For Codex patch rejected, sandbox denial, approval denial, outside of the project, or out-of-workspace write symptoms, inspect cwd, workspace policy, prompt scope, and isolation before retrying. Prefer narrowing the prompt or using managed worktree isolation over loosening sandbox permissions.

## provider comparison

Run equivalent read-only prompts against selected providers. For Agent Bridge behavior analysis, run paired profile "bridge" and profile "bare" tasks where useful; use profile "unblocked" only when the comparison is specifically about workspace-permission reach. All profiles use the lean-only final-output contract. Compare `reviewPacket`, `agent_result` `sections: ["transcript"]`, diagnostics, exit metadata, `profileDiagnostics`, and provider prose as evidence; keep final conclusions and verification responsibility with the main caller.
"#;

const CODE_EXECUTION_RESOURCE: &str = r#"# Agent Bridge Code-Execution-Friendly Delegation

Agent Bridge exposes a small, composable eight-tool surface designed to keep token cost low,
including for callers that drive it from a code-execution or Tool-Search harness.

Principles:
- Poll compactly. `agent_observe` returns a lean envelope (each field once: `agentId`,
  `status`, `isFinal`, `phase`, `progress`, `events`, `nextCursor`, `timedOut`, `next`).
  Use `until: "final"` with `timeoutMs` to block until finality, `limit: 0` for a quick
  state check, and advance `cursor` to read only new transcript events.
- Fetch evidence on demand. `agent_result` returns the review packet and `changedFiles` by
  default. Request `sections: ["stdout","stderr","diff","transcript"]` only when you need
  raw evidence, and page it with `maxBytes`, `stdoutLine`, `stderrLine`, and the transcript
  `cursor`/`limit`. Large logs and diffs stay out of context until you ask for them.
- Keep tool responses lean by default; pass `verbosity: "detailed"` only when you need
  debug metadata (timestamps, launch profile, prompt strategy, diagnostics). Provider
  final output is lean-only; do not spend context on source echo, progress narration,
  generic checklists, or polish.
- Read tool annotations (`readOnlyHint`, `destructiveHint`) to tier and defer the
  diagnostic and control tools when your client supports on-demand tool loading.
- Provider output is evidence only. Run caller-owned verification before claiming the
  original request is complete.
"#;
