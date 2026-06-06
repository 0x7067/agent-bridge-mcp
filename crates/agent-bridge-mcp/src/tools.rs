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
    #[serde(rename = "agent_preview")]
    AgentPreview,
    #[serde(rename = "agent_spawn")]
    AgentSpawn,
    #[serde(rename = "agent_list")]
    AgentList,
    #[serde(rename = "agent_status")]
    AgentStatus,
    #[serde(rename = "agent_wait")]
    AgentWait,
    #[serde(rename = "agent_logs")]
    AgentLogs,
    #[serde(rename = "agent_transcript")]
    AgentTranscript,
    #[serde(rename = "agent_observe")]
    AgentObserve,
    #[serde(rename = "agent_result")]
    AgentResult,
    #[serde(rename = "agent_stop")]
    AgentStop,
    #[serde(rename = "agent_remove")]
    AgentRemove,
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
            "description": "List first-class delegation providers and their agent capabilities.",
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
            "agent_preview",
            "Preview the command that would be run for a provider agent without actually spawning it.",
            &provider_enum,
            &mode_enum,
            &profile_enum,
        ),
        spawn_like_tool(
            "agent_spawn",
            "Start a provider agent. Returns the persisted agentId used by agent_status, agent_wait, agent_logs, agent_transcript, agent_observe, agent_result, agent_stop, and agent_remove.",
            &provider_enum,
            &mode_enum,
            &profile_enum,
        ),
        json!({
            "name": "agent_list",
            "description": "List active and recent provider agents using bounded presentation summaries.",
            "inputSchema": object_schema(json!({
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
            "outputSchema": output_schema_for("agent_list")
        }),
        task_id_tool_with_output("agent_status", "Read one provider agent's lifecycle state."),
        json!({
            "name": "agent_wait",
            "description": "Wait for a provider agent to reach a final state or timeout.",
            "inputSchema": object_schema(json!({"agentId": {"type": "string"}, "timeoutMs": {"type": "number"}}), vec!["agentId"]),
            "outputSchema": output_schema_for("agent_wait")
        }),
        json!({
            "name": "agent_logs",
            "description": "Return capped stdout/stderr log slices for a provider agent.",
            "inputSchema": object_schema(json!({
                "agentId": {"type": "string"},
                "maxBytes": {"type": "number"},
                "stdoutLine": {"type": "number"},
                "stderrLine": {"type": "number"}
            }), vec!["agentId"])
        }),
        json!({
            "name": "agent_transcript",
            "description": "Return bounded normalized transcript events for a provider agent.",
            "inputSchema": object_schema(json!({
                "agentId": {"type": "string"},
                "cursor": {"type": "number"},
                "limit": {"type": "number"}
            }), vec!["agentId"])
        }),
        json!({
            "name": "agent_observe",
            "description": "Observe a provider agent for new transcript/lifecycle events or finalization using bounded long polling.",
            "inputSchema": object_schema(json!({
                "agentId": {"type": "string"},
                "cursor": {"type": "number", "minimum": 0},
                "limit": {"type": "number", "minimum": 1, "maximum": 500},
                "timeoutMs": {"type": "number", "minimum": 0, "maximum": 120000}
            }), vec!["agentId"]),
            "outputSchema": output_schema_for("agent_observe")
        }),
        json!({
            "name": "agent_result",
            "description": "Return final provider-agent metadata, logs, git status, diff, changed files, and exit metadata.",
            "inputSchema": object_schema(json!({"agentId": {"type": "string"}, "maxBytes": {"type": "number"}}), vec!["agentId"]),
            "outputSchema": output_schema_for("agent_result")
        }),
        simple_task_id_tool("agent_stop", "Terminate a running provider agent."),
        simple_task_id_tool(
            "agent_remove",
            "Remove a finished/stopped provider agent. Managed worktree cleanup is mandatory and failure keeps the task record.",
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
        "inputSchema": object_schema(json!({"agentId": {"type": "string"}}), vec!["agentId"])
    })
}

fn task_id_tool_with_output(name: &str, description: &str) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": object_schema(json!({"agentId": {"type": "string"}}), vec!["agentId"]),
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
                "binary": {"type": "object"},
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
                "binary",
                "clients",
                "taskExtensionReadiness",
                "providers",
                "recommendations",
            ],
        ),
        "agent_list" => output_object_schema(
            json!({
                "agents": {"type": "array"},
                "scope": {"type": "string"},
                "limit": {"type": ["integer", "null"]}
            }),
            vec!["agents", "scope"],
        ),
        "agent_status" | "agent_wait" => output_object_schema(
            json!({
                "agentId": {"type": "string"},
                "status": {"type": "string"},
                "isFinal": {"type": "boolean"},
                "presentation": {"type": "object"},
                "nextActions": {"type": "array"}
            }),
            vec!["agentId", "status", "isFinal", "presentation"],
        ),
        "agent_observe" => output_object_schema(
            json!({
                "agentId": {"type": "string"},
                "status": {"type": "string"},
                "isFinal": {"type": "boolean"},
                "agent": {"type": "object"},
                "presentation": {"type": "object"},
                "progress": {"type": "object"},
                "events": {"type": "array"},
                "nextCursor": {"type": "integer"},
                "timedOut": {"type": "boolean"},
                "nextActions": {"type": "array"}
            }),
            vec![
                "agentId",
                "status",
                "isFinal",
                "agent",
                "presentation",
                "progress",
                "events",
                "nextCursor",
                "timedOut",
                "nextActions",
            ],
        ),
        "agent_result" => output_object_schema(
            json!({
                "agentId": {"type": "string"},
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
                "agentId",
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
