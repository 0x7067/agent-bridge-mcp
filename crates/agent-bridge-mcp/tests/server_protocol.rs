use agent_bridge_mcp::mcp::{JsonRpcId, JsonRpcRequest};
use agent_bridge_mcp::server::handle_request;

fn request(method: &str, id: i64, params: serde_json::Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(JsonRpcId::Number(id)),
        method: method.to_string(),
        params: Some(params),
    }
}

#[tokio::test]
async fn initialize_returns_public_server_info() {
    let response = handle_request(request("initialize", 1, serde_json::json!({}))).await;
    let result = response.unwrap().result.unwrap();

    assert_eq!(result["protocolVersion"], "2024-11-05");
    assert_eq!(result["capabilities"], serde_json::json!({ "tools": {} }));
    assert_eq!(result["serverInfo"]["name"], "agent-bridge-mcp");
}

#[tokio::test]
async fn tools_list_returns_current_public_tool_names() {
    let response = handle_request(request("tools/list", 2, serde_json::json!({}))).await;
    let result = response.unwrap().result.unwrap();
    let names: Vec<_> = result["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();

    assert_eq!(
        names,
        vec![
            "providers_list",
            "providers_check",
            "task_preview",
            "task_spawn",
            "task_list",
            "task_status",
            "task_wait",
            "task_logs",
            "task_result",
            "task_stop",
            "task_remove"
        ]
    );
}

#[tokio::test]
async fn initialized_and_unknown_notifications_are_ignored() {
    let initialized = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: None,
        method: "notifications/initialized".to_string(),
        params: Some(serde_json::json!({})),
    };
    let unknown = JsonRpcRequest {
        method: "notifications/unknown".to_string(),
        ..initialized.clone()
    };

    assert!(handle_request(initialized).await.is_none());
    assert!(handle_request(unknown).await.is_none());
}

#[tokio::test]
async fn unknown_methods_return_json_rpc_method_not_found() {
    let response = handle_request(request("missing/method", 3, serde_json::json!({})))
        .await
        .unwrap();
    let error = response.error.unwrap();

    assert_eq!(error.code, -32601);
    assert_eq!(error.message, "Method not found: missing/method");
}

#[tokio::test]
async fn providers_list_returns_tool_json_payload() {
    let response = handle_request(request(
        "tools/call",
        4,
        serde_json::json!({ "name": "providers_list", "arguments": {} }),
    ))
    .await
    .unwrap();
    let result = response.result.unwrap();
    let payload: serde_json::Value =
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();

    assert_eq!(result["isError"], false);
    assert_eq!(
        payload["providers"]["codex"]["supportsWorktreeIsolation"],
        true
    );
}

#[tokio::test]
async fn task_preview_rejects_unknown_public_fields() {
    let response = handle_request(request(
        "tools/call",
        5,
        serde_json::json!({
            "name": "task_preview",
            "arguments": { "provider": "codex", "mode": "review", "prompt": "x", "maxTurns": 2 }
        }),
    ))
    .await
    .unwrap();
    let result = response.result.unwrap();

    assert_eq!(result["isError"], true);
    assert!(
        result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Unknown argument")
    );
}
