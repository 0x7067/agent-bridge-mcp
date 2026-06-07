use crate::domain::{
    Isolation, LaunchProfile, ProviderKind, RetryPolicy, TaskMode, launch_profiles, provider_names,
    task_modes,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolName {
    #[serde(rename = "providers_list")]
    ProvidersList,
    #[serde(rename = "doctor")]
    Doctor,
    #[serde(rename = "agent_spawn")]
    AgentSpawn,
    #[serde(rename = "agent_list")]
    AgentList,
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
    #[serde(default)]
    pub dry_run: Option<bool>,
    #[serde(default)]
    pub retry_policy: Option<RetryPolicy>,
}

pub fn tool_definitions() -> Vec<Value> {
    let provider_enum = provider_names();
    let mode_enum = task_modes();
    let profile_enum = launch_profiles();
    vec![
        json!({
            "name": "providers_list",
            "description": "List first-class delegation providers and their agent capabilities.",
            "inputSchema": object_schema(json!({}), Vec::<&str>::new()),
            "annotations": read_only_annotations("List delegation providers")
        }),
        json!({
            "name": "doctor",
            "description": "Diagnose Agent Bridge setup, workspace, state, client config, binary freshness, and provider/host-runner readiness. Set focus: \"providers\" for a readiness-only check (replaces the former providers_check tool); add smoke: true to startup-verify launchability.",
            "inputSchema": object_schema(json!({
                "focus": {"type": "string", "enum": ["all", "providers"], "description": "\"all\" (default) runs full setup diagnostics; \"providers\" runs only provider readiness."},
                "smoke": {"type": "boolean"},
                "timeoutMs": {"type": "number", "description": "Per-provider smoke budget when smoke is requested."},
                "providers": {"type": "array", "items": {"type": "string", "enum": provider_enum}},
                "aggregateTimeoutMs": {"type": "integer", "minimum": 1, "maximum": 120000},
                "providerTimeoutMs": {
                    "type": "object",
                    "propertyNames": {"enum": provider_enum},
                    "additionalProperties": {"type": "integer", "minimum": 1, "maximum": 90000}
                },
                "cwd": {"type": "string", "description": "Workspace directory to validate against configured workspace roots."}
            }), Vec::<&str>::new()),
            "outputSchema": output_schema_for("doctor"),
            "annotations": read_only_annotations("Diagnose Agent Bridge setup")
        }),
        json!({
            "name": "agent_spawn",
            "description": "Start a provider agent. Primary follow-ups are agent_observe for progress and agent_result for final evidence. Set dryRun: true to preview the command, cwd, environment, profile, and isolation without spawning (replaces the former agent_preview tool).",
            "inputSchema": object_schema(spawn_properties(&provider_enum, &mode_enum, &profile_enum), vec!["provider", "mode", "prompt"]),
            "annotations": {"title": "Start a provider agent", "readOnlyHint": false, "destructiveHint": false, "openWorldHint": true}
        }),
        json!({
            "name": "agent_list",
            "description": "List active and recent provider agents as lean summaries (identity, status, phase, progress, primary next action).",
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
            "outputSchema": output_schema_for("agent_list"),
            "annotations": read_only_annotations("List provider agents")
        }),
        json!({
            "name": "agent_observe",
            "description": "Primary progress path: observe a provider agent for new transcript/lifecycle events, progress, and next actions. until \"now\" (default) returns state plus new events; until \"final\" blocks until finality or timeoutMs (replaces agent_wait); limit 0 returns state only (replaces agent_status). The events stream is the agent transcript (replaces agent_transcript).",
            "inputSchema": object_schema(json!({
                "agentId": {"type": "string"},
                "until": {"type": "string", "enum": ["now", "final"], "description": "\"now\" (default) returns immediately with state and new events; \"final\" blocks until the agent is final or timeoutMs elapses."},
                "cursor": {"type": "number", "minimum": 0},
                "limit": {"type": "number", "minimum": 0, "maximum": 500, "description": "Max transcript events to return; 0 returns lifecycle state without events."},
                "timeoutMs": {"type": "number", "minimum": 0, "maximum": 120000},
                "verbosity": {"type": "string", "enum": ["compact", "detailed"], "description": "\"compact\" (default) returns the lean envelope; \"detailed\" adds debug metadata."}
            }), vec!["agentId"]),
            "outputSchema": output_schema_for("agent_observe"),
            "annotations": read_only_annotations("Observe a provider agent")
        }),
        json!({
            "name": "agent_result",
            "description": "Primary final evidence path: return the review packet and changed files by default; request raw evidence sections on demand. sections selects [\"summary\",\"stdout\",\"stderr\",\"transcript\",\"diff\",\"changedFiles\"] (default [\"summary\",\"changedFiles\"]); request [\"stdout\",\"stderr\"] for the former agent_logs evidence.",
            "inputSchema": object_schema(json!({
                "agentId": {"type": "string"},
                "sections": {
                    "type": "array",
                    "items": {"type": "string", "enum": ["summary", "stdout", "stderr", "transcript", "diff", "changedFiles"]},
                    "description": "Evidence sections to include. Defaults to [\"summary\",\"changedFiles\"]."
                },
                "maxBytes": {"type": "number"},
                "stdoutLine": {"type": "number"},
                "stderrLine": {"type": "number"},
                "cursor": {"type": "number", "minimum": 0, "description": "Transcript cursor when the transcript section is requested."},
                "limit": {"type": "number", "minimum": 1, "maximum": 500, "description": "Max transcript events when the transcript section is requested."},
                "verbosity": {"type": "string", "enum": ["compact", "detailed"]}
            }), vec!["agentId"]),
            "outputSchema": output_schema_for("agent_result"),
            "annotations": read_only_annotations("Inspect provider agent result")
        }),
        json!({
            "name": "agent_stop",
            "description": "Control surface: terminate a running provider agent when it is no longer useful. The stopped agent remains inspectable.",
            "inputSchema": object_schema(json!({"agentId": {"type": "string"}}), vec!["agentId"]),
            "annotations": {"title": "Stop a provider agent", "readOnlyHint": false, "destructiveHint": true, "idempotentHint": true, "openWorldHint": false}
        }),
        json!({
            "name": "agent_remove",
            "description": "Cleanup surface: remove a finished/stopped provider agent after result inspection; managed worktree cleanup failure keeps the agent record.",
            "inputSchema": object_schema(json!({"agentId": {"type": "string"}}), vec!["agentId"]),
            "annotations": {"title": "Remove a provider agent", "readOnlyHint": false, "destructiveHint": true, "idempotentHint": true, "openWorldHint": false}
        }),
    ]
}

fn spawn_properties(provider_enum: &[&str], mode_enum: &[&str], profile_enum: &[&str]) -> Value {
    json!({
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
        "profile": {"type": "string", "enum": profile_enum},
        "dryRun": {"type": "boolean", "description": "Preview the launch (command, cwd, environment, profile, isolation) without spawning."}
    })
}

fn read_only_annotations(title: &str) -> Value {
    json!({
        "title": title,
        "readOnlyHint": true,
        "destructiveHint": false,
        "idempotentHint": true,
        "openWorldHint": false
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
        "agent_observe" => output_object_schema(
            json!({
                "agentId": {"type": "string"},
                "status": {"type": "string"},
                "isFinal": {"type": "boolean"},
                "phase": {"type": "string"},
                "progress": {"type": "object"},
                "events": {"type": "array"},
                "nextCursor": {"type": "integer"},
                "timedOut": {"type": "boolean"},
                "next": {"type": "array"}
            }),
            // events/nextCursor/timedOut are only present on the events-returning
            // path; the state-only reads (limit:0, until:"final") legitimately omit
            // them, so they are optional rather than required.
            vec!["agentId", "status", "isFinal", "phase", "progress", "next"],
        ),
        "agent_result" => output_object_schema(
            json!({
                "agentId": {"type": "string"},
                "status": {"type": "string"},
                "isFinal": {"type": "boolean"},
                "phase": {"type": "string"},
                "reviewPacket": {"type": "object"},
                "changedFiles": {"type": "array"},
                "next": {"type": "array"}
            }),
            vec!["agentId", "status", "isFinal", "reviewPacket", "next"],
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
