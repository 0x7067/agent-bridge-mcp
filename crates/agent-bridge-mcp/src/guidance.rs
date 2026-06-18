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
            "Delegate read-only review.",
        ),
        prompt_definition(
            "agent_bridge_delegate_implementation",
            "Delegate isolated implementation.",
        ),
        prompt_definition(
            "agent_bridge_inspect_result",
            "Inspect a finished task.",
        ),
        prompt_definition(
            "agent_bridge_recover_stalled_task",
            "Recover a stalled task.",
        ),
        prompt_definition(
            "agent_bridge_claude_host_lifecycle",
            "Operate Claude host runner.",
        ),
        prompt_definition(
            "agent_bridge_dogfood_workflows",
            "Run dogfood workflows.",
        ),
        prompt_definition(
            "agent_bridge_compare_providers",
            "Compare providers.",
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
            "Caller workflow",
        ),
        resource_definition(
            SAFETY_URI,
            "safety",
            "Safety guidance",
        ),
        resource_definition(
            PROVIDER_CAPABILITIES_URI,
            "provider-capabilities",
            "Provider capabilities",
        ),
        resource_definition(
            CLAUDE_HOST_LIFECYCLE_URI,
            "claude-host-lifecycle",
            "Claude host lifecycle",
        ),
        resource_definition(
            DOGFOOD_WORKFLOWS_URI,
            "dogfood-workflows",
            "Dogfood workflows",
        ),
        resource_definition(
            CODE_EXECUTION_URI,
            "code-execution",
            "Code-execution guidance",
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

Use Agent Bridge when a separate coding agent can help with research, review, commands, or isolated implementation.

Primary flow:
1. Call `doctor` when setup, workspace, provider, host-runner, client config, or binary freshness is uncertain. Use `focus: "providers"` for readiness-only checks and `smoke: true` for startup proof.
2. Call `agent_spawn`; use `dryRun: true` to preview launch. Every provider receives the lean-only final-output contract.
3. Call `agent_observe` with a bounded timeout; `until: "final"` waits, `limit: 0` returns state, and `events` is the transcript.
4. Once final, call `agent_result` for `reviewPacket`, `changedFiles`, exit metadata, and `next`; request stdout/stderr/diff/transcript sections only when needed.
5. Treat provider output as evidence, not final verification.
6. Call `agent_remove` only after inspecting any managed worktree.

Notes:
- Prefer a launchable provider different from the caller unless the task needs same-provider behavior.
- `doctor.clients`, `doctor.binary`, and `doctor.taskExtensionReadiness` are read-only diagnostics; follow shell recommendations when host-side proof is needed.
- Use `AGENT_BRIDGE_WORKSPACES` for workspace policy; `AGENT_BRIDGE_STATE_DIR` is optional.
- Tool responses are lean by default; pass `verbosity: "detailed"` on `agent_observe`/`agent_result` for debug metadata.
- Use `agent_list` for active agents and final agents whose result has not been inspected.
- Provider agents are not interactive or resumable in v1.

Self-guided clients should read `initialize.instructions`, `structuredContent`, output schemas, and `next`. Protocol-level MCP Tasks are separate; use `agent_spawn`, `agent_list`, and the stable `agent_*` lifecycle here.
"#;

const SAFETY_RESOURCE: &str = r#"# Agent Bridge Safety Guidance

- Keep the main caller responsible for project gates and final claims.
- Run relevant tests, lint, typecheck, build, config, or OpenSpec checks before claiming completion.
- Use `research`/`review` for read-only second opinions; use `implement` with `isolation: "worktree"` for inspectable edits.
- Use `command` only for bounded work with explicit evidence.
- Do not remove a managed worktree until final result, git status, diff, and changed files have been inspected.
- For stalls, use bounded `agent_observe` (`until: "final"` or `limit: 0`) and stop only when no longer useful.
- Stop or ignore agents after useful evidence; do not wait for source echo, progress narration, generic checklists, or polish.
- Use `agent_result` transcript sections for behavior analysis; it does not replace raw logs or main-thread verification.
- For Codex patch rejected, sandbox denial, approval denial, outside of the project, or out-of-workspace writes, use bounded `agent_observe` and final `agent_result`; inspect cwd, workspace policy, prompt scope, and isolation before retrying.
- Do not loosen Codex sandbox permissions as a reflex or repeat unchanged requests after denial diagnostics.
"#;

const PROVIDER_CAPABILITIES_RESOURCE: &str = r#"# Agent Bridge Provider Capabilities

First-class providers:
- `claude`: Claude Code ACP; `CLAUDE_ACP_BIN` defaults to `claude-agent`; `CLAUDE_ACP_ARGS` appends args.
- `cursor`: Cursor ACP; `CURSOR_ACP_BIN` required; `CURSOR_ACP_ARGS` appends args.
- `kimi`: Kimi ACP; `KIMI_ACP_BIN` defaults to `kimi acp`; `KIMI_ACP_ARGS` appends args.
- `codex`: Codex ACP; `CODEX_ACP_BIN` required; `CODEX_ACP_ARGS` appends args. For Codex patch rejected, sandbox denial, approval denial, outside of the project, or out-of-workspace writes, use bounded `agent_observe`, final `agent_result`, agent_spawn dryRun:true, and inspect cwd, workspace policy, prompt scope, and isolation strategy before retrying.
- `forge`: Forge ACP; `FORGE_ACP_BIN` required; `FORGE_ACP_ARGS` appends args.
- `antigravity`: Antigravity ACP; `ANTIGRAVITY_ACP_BIN` required; `ANTIGRAVITY_ACP_ARGS` appends args. Version checks prove binary availability only; smoke can fail until auth is available.

Supported modes:
- `research`: read/analyze only.
- `review`: read-only review.
- `implement`: write-capable implementation.
- `command`: bounded command-oriented work.

Use `providers_list` for runtime provider summaries and launch profiles. Use `doctor` focus:"providers" for availability/startup checks. Do not loosen Codex sandbox permissions as a reflex or repeat unchanged requests after denial diagnostics.

Antigravity `research`/`review` pass `--sandbox`, but read-only filesystem enforcement is not verified; treat those modes as prompt-enforced unless local evidence proves more.

Native-client presentation:
- `providers_list` reports supported actions plus a non-blocking `readiness` snapshot.
- Static discovery starts stale; use `doctor` focus:"providers" to refresh.
- Version-only checks keep `launchable: false` until task-path smoke succeeds.
- `reply` and `resume` are unsupported in v1; render them unavailable, not failed.
"#;

const CLAUDE_HOST_LIFECYCLE_RESOURCE: &str = r#"# Claude Host Runner Lifecycle

Use `agent-bridge-mcp claude-host-runner <socket>` when Claude calls need host access, such as Keychain-backed auth, that the sandboxed MCP server lacks.

Lifecycle:
1. Start the runner outside the sandbox with the MCP server's `AGENT_BRIDGE_WORKSPACES`.
2. Confirm readiness with `doctor`; use host-runner `ping` or Claude-only `doctor` focus:"providers" smoke for follow-up.
3. Configure the MCP server with `AGENT_BRIDGE_CLAUDE_HOST_SOCKET`.
4. On `workspace_policy_mismatch`, align `AGENT_BRIDGE_WORKSPACES` and restart.
5. Stop with SIGTERM or SIGINT so active Claude children terminate.
6. Treat `host_runner_unavailable` as setup to inspect, not permission for silent fallback.

Socket behavior:
- The socket directory must be owner-only.
- Remove a stale socket only after probing that no runner is listening.
- A live socket makes startup fail without unlinking it.
"#;

const DOGFOOD_WORKFLOWS_RESOURCE: &str = r#"# Agent Bridge Dogfood Workflows

Reproducible local operator checks; live provider execution stays opt-in and outside default CI.

## read-only review

Use `agent_spawn` mode `review`/`research`, `isolation: "none"`, a small prompt, and bounded timeout. Use `agent_observe` as the primary progress path, then inspect `agent_result.reviewPacket`; request transcript/stdout/stderr/diff sections only for raw evidence. Use `until: "final"` when only finality matters.

## active task list

Use default `agent_list` as an attention inbox: active agents first, then final uninspected agents. Each record is a lean summary; use filters for inspected history.

## isolated implementation

Use `agent_spawn` mode `implement` with `isolation: "worktree"`. After completion, inspect `reviewPacket`, request diff/changedFiles as needed, verify in the main caller, then call `agent_remove` only after reviewing the managed worktree.

## stalled-task recovery

Use bounded `agent_observe`. If it gives no useful evidence, request stdout/stderr/transcript sections with cursors, then `agent_observe` `limit: 0` for state. Call `agent_stop` only when no longer useful, then inspect final `agent_result`.

For Codex patch rejected, sandbox denial, approval denial, outside of the project, or out-of-workspace writes, inspect cwd, workspace policy, prompt scope, and isolation before retrying. Prefer narrowing or managed worktree isolation over loosening sandbox permissions.

## provider comparison

Run equivalent read-only prompts against selected providers. For bridge behavior, pair "bridge"/"bare" profiles where useful; use "unblocked" only for workspace-permission reach. All profiles use the lean-only final-output contract. Compare `reviewPacket`, transcript evidence, diagnostics, exit metadata, `profileDiagnostics`, and provider prose; keep verification with the main caller.
"#;

const CODE_EXECUTION_RESOURCE: &str = r#"# Agent Bridge Code-Execution-Friendly Delegation

Agent Bridge keeps token cost low for code-execution or Tool-Search harness callers.

Principles:
- Poll compactly. `agent_observe` returns each field once; use `until: "final"` with `timeoutMs`, `limit: 0` for state, and `cursor` for new transcript events.
- Fetch evidence on demand. `agent_result` defaults to review packet plus `changedFiles`; request `sections: ["stdout","stderr","diff","transcript"]` only for raw evidence and page with `maxBytes` or line/cursor limits. Large logs and diffs stay out of context until requested.
- Keep tool responses lean by default; `verbosity: "detailed"` is only for debug metadata. Provider final output is lean-only; avoid source echo, progress narration, generic checklists, or polish.
- Read tool annotations (`readOnlyHint`, `destructiveHint`) when your client supports on-demand tool loading.
- Provider output is evidence only. Run caller-owned verification before claiming completion.
"#;
