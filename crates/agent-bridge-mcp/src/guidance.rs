use serde::Deserialize;
use serde_json::{Value, json};

const CALLER_WORKFLOW_URI: &str = "agent-bridge://guidance/caller-workflow";
const SAFETY_URI: &str = "agent-bridge://guidance/safety";
const PROVIDER_CAPABILITIES_URI: &str = "agent-bridge://guidance/provider-capabilities";

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
    ]
}

pub fn read_resource(params: Value) -> Result<Value, String> {
    let params: ResourceReadParams = serde_json::from_value(params)
        .map_err(|error| format!("Resource not found: invalid params: {error}"))?;

    let text = match params.uri.as_str() {
        CALLER_WORKFLOW_URI => CALLER_WORKFLOW_RESOURCE,
        SAFETY_URI => SAFETY_RESOURCE,
        PROVIDER_CAPABILITIES_URI => PROVIDER_CAPABILITIES_RESOURCE,
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
1. Call providers_check when provider readiness is uncertain.
2. Use task_spawn with mode "review" or "research" and a bounded prompt.
3. Poll task_wait with a bounded timeout, then task_logs if more progress detail is needed.
4. Read task_result once the task is final and inspect logs, gitStatus, diff, changedFiles, exit metadata, and errorType.
5. Treat provider output as evidence; the main caller remains responsible for deciding whether findings are valid."#;

const IMPLEMENTATION_PROMPT: &str = r#"Use Agent Bridge for isolated implementation work.

Suggested flow:
1. Call providers_check first if provider readiness is uncertain.
2. Call task_preview when command flags, cwd, environment, or isolation need inspection.
3. Call task_spawn with mode "implement", a clear task prompt, cwd under an allowed workspace, and isolation "worktree" by default.
4. Use task_wait, task_logs, and task_status to monitor the task without assuming it is finished.
5. When final, call task_result and inspect the report, logs, gitStatus, diff, changedFiles, exit metadata, and errorType.
6. The main caller remains responsible for running relevant tests, lint, typecheck, build, or OpenSpec validation before claiming work complete.
7. Call task_remove only after the managed worktree has been inspected and cleanup is intentional."#;

const INSPECT_RESULT_PROMPT: &str = r#"Inspect an Agent Bridge task result.

Use task_result for the final payload, then review:
- status and errorType
- stdout/stderr excerpts and diagnostics
- gitStatus, diff, and changedFiles
- provider exit metadata

Do not treat provider completion as final verification. The main caller remains responsible for checking the result against the original request and running the smallest relevant proof before claiming completion."#;

const RECOVER_STALLED_PROMPT: &str = r#"Recover a stalled Agent Bridge task.

Suggested flow:
1. Call task_wait with a short bounded timeout.
2. Call task_logs with stdoutLine and stderrLine cursors to inspect new output without rereading the whole log.
3. Call task_status to confirm whether the process is still active.
4. If it is no longer useful, call task_stop.
5. Call task_result after stopping or completion to inspect logs, diagnostics, exit metadata, and partial git state.
6. Decide in the main caller whether to discard, rerun with a narrower prompt, or manually continue."#;

const CALLER_WORKFLOW_RESOURCE: &str = r#"# Agent Bridge Caller Workflow

Use Agent Bridge when a separate coding agent can provide useful research, review, command execution, or isolated implementation work.

Recommended flow:
1. Call `providers_check` to catch missing or misconfigured provider CLIs. Use smoke checks when debugging startup.
2. Call `task_preview` when cwd, flags, environment, prompt transport, or worktree isolation need inspection.
3. Call `task_spawn` for the real delegated task.
4. Call `task_wait` with a bounded timeout. If it times out, call `task_logs` with line cursors to inspect progress.
5. Once final, call `task_result` for logs, git status, diff, changed files, exit metadata, diagnostics, and `errorType`.
6. Treat provider output as evidence for the main caller, not as final verification.
7. Call `task_remove` intentionally after any managed worktree has been inspected.
"#;

const SAFETY_RESOURCE: &str = r#"# Agent Bridge Safety Guidance

- Keep the main caller responsible for project gates and final claims.
- Run relevant tests, lint, typecheck, build, config validation, or OpenSpec validation before saying work is complete.
- Prefer `research` and `review` modes for read-only second opinions.
- Prefer `implement` with `isolation: "worktree"` so provider edits can be inspected before integration.
- Use `command` mode only for bounded command-oriented work with explicit expected evidence.
- Do not remove a managed worktree until the final result, git status, diff, and changed files have been inspected.
- If a task stalls, use bounded `task_wait`, incremental `task_logs`, and `task_stop` rather than waiting indefinitely.
"#;

const PROVIDER_CAPABILITIES_RESOURCE: &str = r#"# Agent Bridge Provider Capabilities

First-class providers:
- `claude`: local Claude Code through `claude-p` by default, or native `claude -p` when configured.
- `cursor`: local Cursor Agent through `cursor-agent -p`.
- `kimi`: local Pi/Kimi through `pi -p`.
- `codex`: local Codex through `codex exec`.

Supported modes:
- `research`: read/analyze only.
- `review`: read-only review.
- `implement`: write-capable implementation.
- `command`: bounded command-oriented work.

Use `providers_list` for the authoritative runtime provider summary and `providers_check` for availability and startup checks.
"#;
