use crate::domain::{Isolation, ProviderKind, TaskMode, provider_names, task_modes};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolName {
    #[serde(rename = "providers_list")]
    ProvidersList,
    #[serde(rename = "providers_check")]
    ProvidersCheck,
    #[serde(rename = "task_preview")]
    TaskPreview,
    #[serde(rename = "task_spawn")]
    TaskSpawn,
    #[serde(rename = "task_list")]
    TaskList,
    #[serde(rename = "task_status")]
    TaskStatus,
    #[serde(rename = "task_wait")]
    TaskWait,
    #[serde(rename = "task_logs")]
    TaskLogs,
    #[serde(rename = "task_result")]
    TaskResult,
    #[serde(rename = "task_stop")]
    TaskStop,
    #[serde(rename = "task_remove")]
    TaskRemove,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolCallParams {
    pub name: ToolName,
    #[serde(default)]
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskPreviewInput {
    pub provider: ProviderKind,
    pub mode: TaskMode,
    pub prompt: String,
    pub title: Option<String>,
    pub cwd: Option<String>,
    pub timeout_seconds: Option<i64>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub thinking: Option<String>,
    pub isolation: Option<Isolation>,
    pub worktree_name: Option<String>,
}

pub fn tool_definitions() -> Vec<Value> {
    let provider_enum = provider_names();
    let mode_enum = task_modes();
    vec![
        json!({
            "name": "providers_list",
            "description": "List first-class delegation providers and their task capabilities.",
            "inputSchema": object_schema(json!({}), Vec::<&str>::new())
        }),
        json!({
            "name": "providers_check",
            "description": "Check availability of each provider by running their command with --version, optionally with a startup smoke probe.",
            "inputSchema": object_schema(json!({"smoke": {"type": "boolean"}, "timeoutMs": {"type": "number"}}), Vec::<&str>::new())
        }),
        spawn_like_tool(
            "task_preview",
            "Preview the command that would be run for a task without actually spawning it.",
            &provider_enum,
            &mode_enum,
        ),
        spawn_like_tool(
            "task_spawn",
            "Start a background provider task. Returns immediately; poll task_status/task_logs/task_result using the returned taskId.",
            &provider_enum,
            &mode_enum,
        ),
        json!({
            "name": "task_list",
            "description": "List tracked provider tasks.",
            "inputSchema": object_schema(json!({}), Vec::<&str>::new())
        }),
        simple_task_id_tool("task_status", "Read one task's lifecycle state."),
        json!({
            "name": "task_wait",
            "description": "Wait for a task to reach a final state or timeout.",
            "inputSchema": object_schema(json!({"taskId": {"type": "string"}, "timeoutMs": {"type": "number"}}), vec!["taskId"])
        }),
        json!({
            "name": "task_logs",
            "description": "Return capped stdout/stderr log slices for a task.",
            "inputSchema": object_schema(json!({
                "taskId": {"type": "string"},
                "maxBytes": {"type": "number"},
                "stdoutLine": {"type": "number"},
                "stderrLine": {"type": "number"}
            }), vec!["taskId"])
        }),
        json!({
            "name": "task_result",
            "description": "Return final task metadata, logs, git status, diff, changed files, and exit metadata.",
            "inputSchema": object_schema(json!({"taskId": {"type": "string"}, "maxBytes": {"type": "number"}}), vec!["taskId"])
        }),
        simple_task_id_tool("task_stop", "Terminate a running task."),
        simple_task_id_tool(
            "task_remove",
            "Remove a finished/stopped task. Managed worktree cleanup is mandatory and failure keeps the task record.",
        ),
    ]
}

fn spawn_like_tool(
    name: &str,
    description: &str,
    provider_enum: &[&str],
    mode_enum: &[&str],
) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": object_schema(json!({
            "provider": {"type": "string", "enum": provider_enum},
            "mode": {"type": "string", "enum": mode_enum},
            "prompt": {"type": "string", "description": "Task prompt. Maximum 100 KiB UTF-8."},
            "title": {"type": "string"},
            "cwd": {"type": "string", "description": "Workspace directory under the allowed root."},
            "timeoutSeconds": {"type": "number"},
            "model": {"type": "string"},
            "effort": {"type": "string"},
            "thinking": {"type": "string"},
            "isolation": {"type": "string", "enum": ["none", "worktree"]},
            "worktreeName": {"type": "string"}
        }), vec!["provider", "mode", "prompt"])
    })
}

fn simple_task_id_tool(name: &str, description: &str) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": object_schema(json!({"taskId": {"type": "string"}}), vec!["taskId"])
    })
}

fn object_schema(properties: Value, required: Vec<&str>) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": properties
    })
}
