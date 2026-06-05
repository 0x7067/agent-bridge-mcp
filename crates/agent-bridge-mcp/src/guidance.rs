use serde::Deserialize;
use serde_json::{Value, json};

const CALLER_WORKFLOW_URI: &str = "agent-bridge://guidance/caller-workflow";
const SAFETY_URI: &str = "agent-bridge://guidance/safety";
const PROVIDER_CAPABILITIES_URI: &str = "agent-bridge://guidance/provider-capabilities";
const CLAUDE_HOST_LIFECYCLE_URI: &str = "agent-bridge://guidance/claude-host-lifecycle";
const DOGFOOD_WORKFLOWS_URI: &str = "agent-bridge://guidance/dogfood-workflows";

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
1. Call doctor first when setup, workspace, state, provider, or host-runner readiness is uncertain.
2. Call providers_check when provider readiness needs focused follow-up.
3. Use task_spawn with mode "review" or "research" and a bounded prompt.
4. Poll task_wait with a bounded timeout, then task_logs and task_transcript if more progress detail is needed.
5. Read task_result once the task is final and inspect reviewPacket, transcript evidence, logs, gitStatus, diff, changedFiles, exit metadata, and errorType.
6. Treat provider output as evidence; the main caller remains responsible for deciding whether findings are valid."#;

const IMPLEMENTATION_PROMPT: &str = r#"Use Agent Bridge for isolated implementation work.

Suggested flow:
1. Call doctor first when setup, workspace, state, provider, or host-runner readiness is uncertain.
2. Call providers_check if provider readiness needs focused follow-up.
3. Call task_preview when command flags, cwd, environment, or isolation need inspection.
4. Call task_spawn with mode "implement", a clear task prompt, cwd under an allowed workspace, and isolation "worktree" by default.
5. Use task_list or task_status presentation metadata for native-client summaries, action availability, and active/recent task ordering.
6. Use task_wait, task_logs, task_transcript, and task_status to monitor the task without assuming it is finished.
7. When final, call task_result and inspect the report, transcript evidence, logs, gitStatus, diff, changedFiles, exit metadata, and errorType.
8. The main caller remains responsible for running relevant tests, lint, typecheck, build, or OpenSpec validation before claiming work complete.
9. Call task_remove only after the managed worktree has been inspected and cleanup is intentional."#;

const INSPECT_RESULT_PROMPT: &str = r#"Inspect an Agent Bridge task result.

Use task_result for the final payload, then review:
- reviewPacket as the concise operator summary
- status and errorType
- stdout/stderr excerpts and diagnostics
- transcript availability, final-result evidence, and partial-result evidence
- gitStatus, diff, and changedFiles
- provider exit metadata

Do not treat provider completion as final verification. The main caller remains responsible for checking the result against the original request and running the smallest relevant proof before claiming completion."#;

const RECOVER_STALLED_PROMPT: &str = r#"Recover a stalled Agent Bridge task.

Suggested flow:
1. Call task_wait with a short bounded timeout.
2. Call task_logs with stdoutLine and stderrLine cursors, and task_transcript with cursor/limit, to inspect new output without rereading the whole run.
3. Call task_status to confirm whether the process is still active.
4. If it is no longer useful, call task_stop.
5. Call task_result after stopping or completion to inspect logs, diagnostics, exit metadata, and partial git state.
6. Decide in the main caller whether to discard, rerun with a narrower prompt, or manually continue.

Codex denial symptoms such as "patch rejected", sandbox denial, approval denial, outside of the project, or out-of-workspace writes are prompt-scope or workspace-scope failures to inspect. Use task_wait, task_logs, task_status, and final task_result evidence; inspect cwd, workspace policy, prompt scope, and isolation strategy before retrying. Do not loosen sandbox permissions as a reflex or repeat the same request without understanding the diagnostic."#;

const CLAUDE_HOST_LIFECYCLE_PROMPT: &str = r#"Operate the Claude host runner lifecycle.

Use this when Claude Code auth depends on macOS Keychain or another host resource unavailable to the sandboxed MCP process.

Suggested flow:
1. Start `agent-bridge-mcp claude-host-runner <socket>` outside the Codex sandbox with the same AGENT_BRIDGE_WORKSPACES value used by the MCP server.
2. Call doctor from the MCP client to confirm the server sees the socket, workspace policy, and host-runner status.
3. Use the host-runner `ping` request or a Claude-only providers_check smoke for focused follow-up when doctor reports a host-runner problem.
4. If diagnostics report workspace_policy_mismatch, restart the host runner after updating AGENT_BRIDGE_WORKSPACES.
5. Stop the runner with SIGTERM or SIGINT so active Claude child processes are terminated and reaped.
6. If startup finds a stale socket, let the runner remove it only when the connection probe confirms no live runner is listening.
7. If AGENT_BRIDGE_CLAUDE_HOST_SOCKET is configured but unavailable, do not silently fall back; inspect diagnostics and restart the runner."#;

const DOGFOOD_WORKFLOWS_PROMPT: &str = r#"Run Agent Bridge dogfood workflows.

Suggested workflows:
1. For read-only review, use mode "review" or "research", isolation "none", a small prompt, bounded task_wait, and final task_result review.
2. For isolated implementation, use mode "implement", isolation "worktree", inspect reviewPacket, gitStatus, gitDiff, changedFiles, stdout, stderr, and diagnostics, then run verification in the main caller.
3. For stalled-task recovery, use bounded task_wait, incremental task_logs cursors, task_status, task_stop if needed, and final task_result inspection. For Codex patch rejected, sandbox denial, approval denial, outside of the project, or out-of-workspace write symptoms, inspect cwd, workspace policy, prompt scope, and isolation strategy before retrying.
4. For provider comparison, run equivalent read-only prompts against selected providers, optionally paired as profile "bridge" and profile "bare"; compare task_result, task_transcript, profileDiagnostics, and provider prose, and keep final conclusions in the main caller.

Live provider execution remains opt-in and should not be added to default CI."#;

const COMPARE_PROVIDERS_PROMPT: &str = r#"Compare Agent Bridge providers safely.

Suggested flow:
1. Call providers_check for the selected providers; use smoke only when startup readiness matters.
2. Use task_preview to confirm command shape, cwd, launch strategy, selected profile, profileDiagnostics, and provider options.
3. Spawn equivalent read-only review or research tasks with short prompts and bounded timeouts. Use profile "bridge" for normal Agent Bridge guidance and profile "bare" for compact reduced-configuration experiments.
4. Use task_wait, task_logs, task_transcript, and task_result for each task.
5. Compare reviewPacket, transcript evidence, logs, diagnostics, exit metadata, profileDiagnostics, and provider prose as evidence.
6. Keep correctness decisions and project verification in the main caller."#;

const CALLER_WORKFLOW_RESOURCE: &str = r#"# Agent Bridge Caller Workflow

Use Agent Bridge when a separate coding agent can provide useful research, review, command execution, or isolated implementation work.

Recommended flow:
1. Call `doctor` first when setup, workspace, state, provider, or host-runner readiness is uncertain.
2. Call `providers_check` to catch missing or misconfigured provider CLIs. Use smoke checks when debugging startup.
3. Call `task_preview` when cwd, flags, environment, prompt transport, or worktree isolation need inspection.
4. Call `task_spawn` for the real delegated task.
5. Use `task_list` and `task_status` `presentation` metadata for native-client rendering: active/recent ordering, display titles, status tone, result availability, and structured actions.
6. Render unavailable `reply` and `resume` actions as disabled controls with their reasons; provider tasks are not interactive or resumable in v1.
7. Call `task_wait` with a bounded timeout. If it times out, call `task_logs` with line cursors and `task_transcript` with cursor/limit to inspect progress.
8. Once final, call `task_result` for `reviewPacket`, transcript availability/result evidence, logs, git status, diff, changed files, exit metadata, diagnostics, and `errorType`.
9. Treat provider output and native-feeling completion as evidence for the main caller, not as final verification.
10. Call `task_remove` intentionally after any managed worktree has been inspected. `presentation.actions` may mark cleanup as `unsafe` for managed worktree tasks until result inspection is explicit.
"#;

const SAFETY_RESOURCE: &str = r#"# Agent Bridge Safety Guidance

- Keep the main caller responsible for project gates and final claims.
- Run relevant tests, lint, typecheck, build, config validation, or OpenSpec validation before saying work is complete.
- Prefer `research` and `review` modes for read-only second opinions.
- Prefer `implement` with `isolation: "worktree"` so provider edits can be inspected before integration.
- Use `command` mode only for bounded command-oriented work with explicit expected evidence.
- Do not remove a managed worktree until the final result, git status, diff, and changed files have been inspected.
- If a task stalls, use bounded `task_wait`, incremental `task_logs`, and `task_stop` rather than waiting indefinitely.
- Use `task_transcript` for behavior analysis, provider comparison, and final/partial result evidence; it does not replace raw logs or main-thread verification.
- For Codex patch rejected, sandbox denial, approval denial, outside of the project, or out-of-workspace write symptoms, use `task_wait`, `task_logs`, `task_status`, and final `task_result`; inspect cwd, workspace policy, prompt scope, and isolation before retrying.
- Do not loosen Codex sandbox permissions as a reflex or repeat an unchanged request after denial diagnostics.
"#;

const PROVIDER_CAPABILITIES_RESOURCE: &str = r#"# Agent Bridge Provider Capabilities

First-class providers:
- `claude`: local Claude Code through `claude-p` by default, or native `claude -p` when configured.
- `cursor`: local Cursor Agent through `cursor-agent -p`.
- `kimi`: local Pi/Kimi through `pi -p`.
- `codex`: local Codex through `codex exec`. Codex patch rejected, sandbox denial, approval denial, outside of the project, or out-of-workspace write symptoms should be investigated with `task_wait`, `task_logs`, `task_status`, final `task_result`, `task_preview`, cwd, workspace policy, prompt scope, and isolation strategy before retrying.

Supported modes:
- `research`: read/analyze only.
- `review`: read-only review.
- `implement`: write-capable implementation.
- `command`: bounded command-oriented work.

Use `providers_list` for the authoritative runtime provider summary, including launch profiles and reduced-configuration metadata. Use `providers_check` for availability and startup checks. Do not loosen Codex sandbox permissions as a reflex or repeat an unchanged request after denial diagnostics.

Native-client presentation:
- `providers_list` reports `supportsReply`, `supportsResume`, and `presentationActions` so clients can render supported and unsupported controls without trial-and-error task calls.
- `providers_list` includes a non-blocking `readiness` snapshot. Static discovery starts as `state: "stale"` and `launchable: false`; use `providers_check` to refresh readiness.
- Version-only checks keep `launchable: false` unless a task-path smoke probe succeeds. Smoke-verified providers report `readiness.state: "ready"` and `launchable: true`.
- `reply` and `resume` are unsupported for provider tasks in v1. Clients should render them as unavailable actions with explanatory reasons, not as failed tool calls.
"#;

const CLAUDE_HOST_LIFECYCLE_RESOURCE: &str = r#"# Claude Host Runner Lifecycle

Use `agent-bridge-mcp claude-host-runner <socket>` when Claude provider calls need host access that the sandboxed MCP server does not have, such as macOS Keychain-backed Claude Code auth.

Lifecycle:
1. Start the runner outside the sandbox with the same `AGENT_BRIDGE_WORKSPACES` value as the MCP server.
2. Confirm readiness with `doctor`, then use the host-runner `ping` request or a Claude-only `providers_check` smoke for focused follow-up.
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

Use `task_spawn` with mode `review` or `research`, `isolation: "none"`, a small prompt, and a bounded timeout. Use `task_wait`, then inspect `task_result.reviewPacket`, `task_transcript`, stdout, stderr, diagnostics, git status, diff, changed files, and exit metadata.

## native task presentation

Use `task_list` with default arguments to show active tasks first and recent final tasks second. Read each task's `presentation` object for display title, status tone, result availability, `verificationStatus: "not_verified"`, and structured actions. Use `presentation: false` with `scope: "all"` only when an operator intentionally needs raw full-history registry inspection.

## isolated implementation

Use `task_spawn` with mode `implement` and `isolation: "worktree"`. After completion, inspect `reviewPacket`, `gitStatus`, `gitDiff`, and `changedFiles`; run the relevant verification in the main caller; call `task_remove` only after the managed worktree has been reviewed.

## stalled-task recovery

Use short bounded `task_wait` calls. If the task does not finish, call `task_logs` with `stdoutLine` and `stderrLine` cursors, `task_transcript` with cursor/limit, then `task_status`. Call `task_stop` only when the task is no longer useful, then inspect final `task_result`.

For Codex patch rejected, sandbox denial, approval denial, outside of the project, or out-of-workspace write symptoms, inspect cwd, workspace policy, prompt scope, and isolation before retrying. Prefer narrowing the prompt or using managed worktree isolation over loosening sandbox permissions.

## provider comparison

Run equivalent read-only prompts against selected providers. For Agent Bridge behavior analysis, run paired profile "bridge" and profile "bare" tasks where useful. Compare `reviewPacket`, `task_transcript`, logs, diagnostics, exit metadata, `profileDiagnostics`, and provider prose as evidence; keep final conclusions and verification responsibility with the main caller.
"#;
