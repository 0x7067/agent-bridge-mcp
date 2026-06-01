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
    assert_eq!(
        result["capabilities"],
        serde_json::json!({ "tools": {}, "prompts": {}, "resources": {} })
    );
    assert_eq!(result["serverInfo"]["name"], "agent-bridge-mcp");
}

#[tokio::test]
async fn guidance_prompts_are_listed_and_retrievable() {
    let response = handle_request(request("prompts/list", 2, serde_json::json!({}))).await;
    let result = response.unwrap().result.unwrap();
    let prompts = result["prompts"].as_array().unwrap();
    let names: Vec<_> = prompts
        .iter()
        .map(|prompt| prompt["name"].as_str().unwrap())
        .collect();

    assert_eq!(
        names,
        vec![
            "agent_bridge_delegate_review",
            "agent_bridge_delegate_implementation",
            "agent_bridge_inspect_result",
            "agent_bridge_recover_stalled_task"
        ]
    );

    let response = handle_request(request(
        "prompts/get",
        3,
        serde_json::json!({ "name": "agent_bridge_delegate_implementation" }),
    ))
    .await;
    let result = response.unwrap().result.unwrap();
    let text = result["messages"][0]["content"]["text"].as_str().unwrap();

    assert!(text.contains("task_spawn"));
    assert!(text.contains("task_result"));
    assert!(text.contains("main caller remains responsible"));
}

#[tokio::test]
async fn unknown_guidance_prompt_returns_invalid_params() {
    let response = handle_request(request(
        "prompts/get",
        4,
        serde_json::json!({ "name": "missing_prompt" }),
    ))
    .await
    .unwrap();
    let error = response.error.unwrap();

    assert_eq!(error.code, -32602);
    assert!(error.message.contains("Unknown prompt"));
}

#[tokio::test]
async fn guidance_resources_are_listed_and_read_from_allowlist() {
    let response = handle_request(request("resources/list", 5, serde_json::json!({}))).await;
    let result = response.unwrap().result.unwrap();
    let resources = result["resources"].as_array().unwrap();
    let uris: Vec<_> = resources
        .iter()
        .map(|resource| resource["uri"].as_str().unwrap())
        .collect();

    assert_eq!(
        uris,
        vec![
            "agent-bridge://guidance/caller-workflow",
            "agent-bridge://guidance/safety",
            "agent-bridge://guidance/provider-capabilities"
        ]
    );

    let response = handle_request(request(
        "resources/read",
        6,
        serde_json::json!({ "uri": "agent-bridge://guidance/caller-workflow" }),
    ))
    .await;
    let result = response.unwrap().result.unwrap();
    let content = &result["contents"][0];

    assert_eq!(content["mimeType"], "text/markdown");
    assert_eq!(content["uri"], "agent-bridge://guidance/caller-workflow");
    assert!(
        content["text"]
            .as_str()
            .unwrap()
            .contains("providers_check")
    );
    assert!(content["text"].as_str().unwrap().contains("task_remove"));
}

#[tokio::test]
async fn guidance_resources_reject_non_allowlisted_uris() {
    for uri in [
        "agent-bridge://guidance/missing",
        "file:///etc/passwd",
        "not a uri",
    ] {
        let response = handle_request(request(
            "resources/read",
            7,
            serde_json::json!({ "uri": uri }),
        ))
        .await
        .unwrap();
        let error = response.error.unwrap();

        assert_eq!(error.code, -32002, "{uri}");
        assert!(error.message.contains("Resource not found"), "{uri}");
    }
}

#[tokio::test]
async fn tools_list_returns_current_public_tool_names() {
    let response = handle_request(request("tools/list", 8, serde_json::json!({}))).await;
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
