use crate::domain::{ProviderKind, TaskMode};
use crate::mcp::{JsonRpcId, JsonRpcRequest, JsonRpcResponse};
use crate::router::RouterPolicy;
use crate::router_runtime::{
    RouterPromptError, RouterPromptTurn, RouterUpdateSink, execute_router_turn,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use uuid::Uuid;

const PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum AdapterToolName {
    #[serde(rename = "agent_delegate")]
    AgentDelegate,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdapterToolCallParams {
    name: AdapterToolName,
    #[serde(default)]
    arguments: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DelegateArguments {
    prompt: String,
    cwd: Option<String>,
    mode: Option<TaskMode>,
    timeout_seconds: Option<i64>,
    policy: Option<DelegatePolicy>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DelegatePolicy {
    candidates: Vec<ProviderKind>,
}

pub async fn run() -> io::Result<()> {
    let stdin = io::stdin();
    let mut lines = BufReader::new(stdin).lines();
    let mut stdout = io::stdout();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(request) => handle_request(request).await,
            Err(_) => Some(JsonRpcResponse::error(
                JsonRpcId::Null,
                -32700,
                "Parse error",
            )),
        };
        if let Some(response) = response {
            write_response(&mut stdout, &response).await?;
        }
    }
    Ok(())
}

#[doc(hidden)]
pub async fn handle_request_for_test(request: JsonRpcRequest) -> Option<JsonRpcResponse> {
    handle_request(request).await
}

async fn handle_request(request: JsonRpcRequest) -> Option<JsonRpcResponse> {
    let id = request.id?;
    let response = match request.method.as_str() {
        "initialize" => JsonRpcResponse::result(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "agent-bridge-mcp-adapter", "version": "0.1.0"},
                "instructions": "Use agent_delegate for one routed provider turn. Provider output is evidence; caller verification remains required."
            }),
        ),
        "tools/list" => JsonRpcResponse::result(id, json!({"tools": adapter_tool_definitions()})),
        "tools/call" => JsonRpcResponse::result(
            id,
            call_tool(request.params.unwrap_or_else(|| json!({}))).await,
        ),
        _ => JsonRpcResponse::error(id, -32601, format!("Method not found: {}", request.method)),
    };
    Some(response)
}

async fn call_tool(params: Value) -> Value {
    let parsed = match serde_json::from_value::<AdapterToolCallParams>(params) {
        Ok(parsed) => parsed,
        Err(error) => return tool_error(error.to_string()),
    };
    match parsed.name {
        AdapterToolName::AgentDelegate => match agent_delegate(parsed.arguments).await {
            Ok(result) => tool_json(result),
            Err(error) => tool_error(error),
        },
    }
}

async fn agent_delegate(arguments: Value) -> Result<Value, String> {
    let arguments: DelegateArguments =
        serde_json::from_value(arguments).map_err(|error| error.to_string())?;
    let candidates = arguments
        .policy
        .map(|policy| policy.candidates)
        .unwrap_or_else(|| vec![ProviderKind::Codex, ProviderKind::Claude]);
    let policy = RouterPolicy::new(candidates).map_err(|error| error.to_string())?;
    let turn = RouterPromptTurn {
        session_id: format!("mcp-adapter-{}", Uuid::new_v4().simple()),
        cwd: arguments.cwd,
        prompt: arguments.prompt,
        policy,
        mode: arguments.mode.unwrap_or(TaskMode::Implement),
        timeout_seconds: arguments.timeout_seconds,
    };
    execute_router_turn(turn, RouterUpdateSink::Silent)
        .await
        .map(|result| result.router_result)
        .map_err(router_prompt_error_text)
}

fn router_prompt_error_text(error: RouterPromptError) -> String {
    match error {
        RouterPromptError::InvalidParams(message) | RouterPromptError::Runtime(message) => message,
    }
}

fn adapter_tool_definitions() -> Vec<Value> {
    vec![json!({
        "name": "agent_delegate",
        "description": "Run one routed provider turn and return a terminal router result. The caller remains responsible for verification.",
        "inputSchema": object_schema(
            json!({
                "prompt": {"type": "string"},
                "cwd": {"type": "string"},
                "mode": {"type": "string", "enum": ["review", "implement", "command"]},
                "timeoutSeconds": {"type": "integer", "minimum": 1, "maximum": 1800},
                "policy": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "candidates": {
                            "type": "array",
                            "items": {"type": "string", "enum": ["codex", "claude", "cursor", "kimi", "pi", "forge", "antigravity"]}
                        }
                    },
                    "required": ["candidates"]
                }
            }),
            vec!["prompt"]
        ),
        "annotations": {
            "title": "Delegate to agent",
            "readOnlyHint": false,
            "destructiveHint": false,
            "idempotentHint": false,
            "openWorldHint": true
        }
    })]
}

fn object_schema(properties: Value, required: Vec<&str>) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": properties
    })
}

fn tool_json(value: Value) -> Value {
    json!({
        "content": [{"type": "text", "text": serde_json::to_string(&value).unwrap()}],
        "isError": false
    })
}

fn tool_error(message: String) -> Value {
    json!({
        "content": [{"type": "text", "text": message}],
        "isError": true
    })
}

async fn write_response(stdout: &mut io::Stdout, response: &JsonRpcResponse) -> io::Result<()> {
    let mut line = serde_json::to_vec(response).map_err(io::Error::other)?;
    line.push(b'\n');
    stdout.write_all(&line).await?;
    stdout.flush().await
}
