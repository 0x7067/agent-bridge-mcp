use agent_bridge_mcp::domain::{ProviderKind, TaskMode, TimeoutSeconds, WorktreeName};
use agent_bridge_mcp::mcp::{JsonRpcId, JsonRpcRequest, JsonRpcResponse};
use agent_bridge_mcp::tools::{TaskPreviewInput, ToolCallParams, ToolName};

#[test]
fn json_rpc_request_distinguishes_requests_and_notifications() {
    let request: JsonRpcRequest =
        serde_json::from_str(r#"{"jsonrpc":"2.0","id":7,"method":"tools/list","params":{}}"#)
            .unwrap();
    assert_eq!(request.id, Some(JsonRpcId::Number(7)));
    assert!(request.is_request());

    let notification: JsonRpcRequest = serde_json::from_str(
        r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#,
    )
    .unwrap();
    assert_eq!(notification.id, None);
    assert!(!notification.is_request());
}

#[test]
fn json_rpc_response_serializes_public_shape() {
    let response = JsonRpcResponse::result(
        JsonRpcId::Number(1),
        serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} }
        }),
    );
    let value = serde_json::to_value(response).unwrap();
    assert_eq!(value["jsonrpc"], "2.0");
    assert_eq!(value["id"], 1);
    assert_eq!(value["result"]["protocolVersion"], "2024-11-05");
}

#[test]
fn tool_call_params_parse_known_tool_names() {
    let params: ToolCallParams = serde_json::from_str(
        r#"{"name":"agent_preview","arguments":{"provider":"codex","mode":"review","prompt":"x"}}"#,
    )
    .unwrap();
    assert_eq!(params.name, ToolName::AgentPreview);
}

#[test]
fn agent_preview_input_rejects_unknown_fields() {
    let error = serde_json::from_str::<TaskPreviewInput>(
        r#"{"provider":"codex","mode":"review","prompt":"x","maxTurns":2}"#,
    )
    .unwrap_err();
    assert!(error.to_string().contains("unknown field"));
}

#[test]
fn domain_types_validate_current_public_values() {
    assert_eq!(
        "codex".parse::<ProviderKind>().unwrap(),
        ProviderKind::Codex
    );
    assert_eq!("command".parse::<TaskMode>().unwrap(), TaskMode::Command);
    assert_eq!(TimeoutSeconds::new(Some(999_999)).get(), 1800);
    assert_eq!(TimeoutSeconds::new(Some(-1)).get(), 1);
    assert!(WorktreeName::new("feature.branch_1-ok").is_ok());
    assert!(WorktreeName::new("bad/name").is_err());
}
