use crate::domain::{
    Isolation, LaunchProfile, ProviderKind, TaskMode, launch_profiles, provider_names, task_modes,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolName {
    #[serde(rename = "providers_list")]
    ProvidersList,
    #[serde(rename = "providers_check")]
    ProvidersCheck,
    #[serde(rename = "doctor")]
    Doctor,
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
    #[serde(rename = "task_transcript")]
    TaskTranscript,
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
    #[serde(default, rename = "_meta")]
    pub meta: Option<Value>,
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
    pub profile: Option<LaunchProfile>,
}

pub fn tool_definitions() -> Vec<Value> {
    let provider_enum = provider_names();
    let mode_enum = task_modes();
    let profile_enum = launch_profiles();
    vec![
        json!({
            "name": "providers_list",
            "description": "List first-class delegation providers and their task capabilities.",
            "inputSchema": object_schema(json!({}), Vec::<&str>::new())
        }),
        json!({
            "name": "providers_check",
            "description": "Check availability of each provider by running their command with --version, optionally with a startup smoke probe.",
            "inputSchema": object_schema(json!({
                "smoke": {"type": "boolean"},
                "timeoutMs": {"type": "number"},
                "providers": {"type": "array", "items": {"type": "string", "enum": provider_enum}},
                "aggregateTimeoutMs": {"type": "integer", "minimum": 1, "maximum": 120000},
                "providerTimeoutMs": {
                    "type": "object",
                    "propertyNames": {"enum": provider_enum},
                    "additionalProperties": {"type": "integer", "minimum": 1, "maximum": 90000}
                }
            }), Vec::<&str>::new())
        }),
        json!({
            "name": "doctor",
            "description": "Diagnose Agent Bridge MCP server, workspace, state, provider, and Claude host-runner readiness.",
            "inputSchema": object_schema(json!({
                "smoke": {"type": "boolean"},
                "providers": {"type": "array", "items": {"type": "string", "enum": provider_enum}},
                "aggregateTimeoutMs": {"type": "integer", "minimum": 1, "maximum": 120000},
                "providerTimeoutMs": {
                    "type": "object",
                    "propertyNames": {"enum": provider_enum},
                    "additionalProperties": {"type": "integer", "minimum": 1, "maximum": 90000}
                },
                "cwd": {"type": "string", "description": "Workspace directory to validate against configured workspace roots."}
            }), Vec::<&str>::new()),
            "outputSchema": output_schema_for("doctor")
        }),
        spawn_like_tool(
            "task_preview",
            "Preview the command that would be run for a task without actually spawning it.",
            &provider_enum,
            &mode_enum,
            &profile_enum,
        ),
        spawn_like_tool(
            "task_spawn",
            "Start a background provider task. Returns immediately; poll task_status/task_logs/task_result using the returned taskId.",
            &provider_enum,
            &mode_enum,
            &profile_enum,
        ),
        json!({
            "name": "task_list",
            "description": "List tracked provider tasks.",
            "inputSchema": object_schema(json!({
                "presentation": {
                    "type": "boolean",
                    "description": "Optimize the list for native-client task presentation. Defaults to true with active/recent ordering and a bounded limit."
                },
                "scope": {"type": "string", "enum": ["active_recent", "all"]},
                "status": {
                    "type": "array",
                    "items": {"type": "string", "enum": ["queued", "running", "succeeded", "failed", "stopped", "failed_stale", "removed"]}
                },
                "provider": {"type": "array", "items": {"type": "string", "enum": provider_enum}},
                "mode": {"type": "array", "items": {"type": "string", "enum": mode_enum}},
                "cwd": {"type": "string"},
                "titleContains": {"type": "string"},
                "limit": {"type": "integer", "minimum": 1, "maximum": 100}
            }), Vec::<&str>::new()),
            "outputSchema": output_schema_for("task_list")
        }),
        task_id_tool_with_output("task_status", "Read one task's lifecycle state."),
        json!({
            "name": "task_wait",
            "description": "Wait for a task to reach a final state or timeout.",
            "inputSchema": object_schema(json!({"taskId": {"type": "string"}, "timeoutMs": {"type": "number"}}), vec!["taskId"]),
            "outputSchema": output_schema_for("task_wait")
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
            "name": "task_transcript",
            "description": "Return bounded normalized transcript events for a task.",
            "inputSchema": object_schema(json!({
                "taskId": {"type": "string"},
                "cursor": {"type": "number"},
                "limit": {"type": "number"}
            }), vec!["taskId"])
        }),
        json!({
            "name": "task_result",
            "description": "Return final task metadata, logs, git status, diff, changed files, and exit metadata.",
            "inputSchema": object_schema(json!({"taskId": {"type": "string"}, "maxBytes": {"type": "number"}}), vec!["taskId"]),
            "outputSchema": output_schema_for("task_result")
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
    profile_enum: &[&str],
) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": object_schema(json!({
            "provider": {"type": "string", "enum": provider_enum},
            "mode": {"type": "string", "enum": mode_enum},
            "prompt": {"type": "string", "description": "Task prompt. Maximum 100 KiB UTF-8."},
            "title": {"type": "string"},
            "cwd": {"type": "string", "description": "Workspace directory under a configured workspace root."},
            "timeoutSeconds": {"type": "number"},
            "model": {"type": "string"},
            "effort": {"type": "string"},
            "thinking": {"type": "string"},
            "isolation": {"type": "string", "enum": ["none", "worktree"]},
            "worktreeName": {"type": "string"},
            "profile": {"type": "string", "enum": profile_enum}
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

fn task_id_tool_with_output(name: &str, description: &str) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": object_schema(json!({"taskId": {"type": "string"}}), vec!["taskId"]),
        "outputSchema": output_schema_for(name)
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

fn output_schema_for(name: &str) -> Value {
    match name {
        "doctor" => output_object_schema(
            json!({
                "summary": {"type": "object"},
                "server": {"type": "object"},
                "workspace": {"type": "object"},
                "state": {"type": "object"},
                "clients": {"type": "object"},
                "taskExtensionReadiness": {"type": "object"},
                "providers": {"type": "object"},
                "launchReadiness": {"type": "object"},
                "claudeHostRunner": {"type": "object"},
                "recommendations": {"type": "array"}
            }),
            vec![
                "summary",
                "server",
                "workspace",
                "state",
                "clients",
                "taskExtensionReadiness",
                "providers",
                "recommendations",
            ],
        ),
        "task_list" => output_object_schema(
            json!({
                "tasks": {"type": "array"},
                "presentation": {"type": "boolean"},
                "scope": {"type": "string"},
                "limit": {"type": ["integer", "null"]}
            }),
            vec!["tasks", "presentation", "scope"],
        ),
        "task_status" | "task_wait" => output_object_schema(
            json!({
                "taskId": {"type": "string"},
                "status": {"type": "string"},
                "isFinal": {"type": "boolean"},
                "presentation": {"type": "object"},
                "nextActions": {"type": "array"}
            }),
            vec!["taskId", "status", "isFinal", "presentation"],
        ),
        "task_result" => output_object_schema(
            json!({
                "taskId": {"type": "string"},
                "status": {"type": "string"},
                "isFinal": {"type": "boolean"},
                "presentation": {"type": "object"},
                "nextActions": {"type": "array"},
                "reviewPacket": {"type": "object"},
                "stdout": {"type": "string"},
                "stderr": {"type": "string"},
                "changedFiles": {"type": "array"}
            }),
            vec![
                "taskId",
                "status",
                "isFinal",
                "presentation",
                "reviewPacket",
            ],
        ),
        _ => output_object_schema(json!({}), Vec::<&str>::new()),
    }
}

fn output_object_schema(properties: Value, required: Vec<&str>) -> Value {
    json!({
        "type": "object",
        "additionalProperties": true,
        "required": required,
        "properties": properties
    })
}
