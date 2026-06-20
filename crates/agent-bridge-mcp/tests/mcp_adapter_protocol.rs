use agent_bridge_mcp::mcp::{JsonRpcId, JsonRpcRequest};
use agent_bridge_mcp::mcp_adapter::handle_request_for_test;
use serde_json::{Value, json};

fn request(method: &str, id: i64, params: Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(JsonRpcId::Number(id)),
        method: method.to_string(),
        params: Some(params),
    }
}

#[tokio::test]
async fn initialize_advertises_minimal_mcp_adapter() {
    let response = handle_request_for_test(request("initialize", 1, json!({})))
        .await
        .unwrap();
    let result = response.result.unwrap();

    assert_eq!(result["protocolVersion"], "2024-11-05");
    assert_eq!(result["capabilities"], json!({"tools": {}}));
    assert_eq!(result["serverInfo"]["name"], "agent-bridge-mcp-adapter");
    assert!(
        result["instructions"]
            .as_str()
            .unwrap()
            .contains("agent_delegate")
    );
}

#[tokio::test]
async fn tools_list_contains_delegate_and_no_lifecycle_tools() {
    let response = handle_request_for_test(request("tools/list", 2, json!({})))
        .await
        .unwrap();
    let tools = response.result.unwrap()["tools"]
        .as_array()
        .unwrap()
        .clone();
    let names: Vec<_> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();

    assert_eq!(names, vec!["agent_delegate"]);
    assert!(!names.contains(&"agent_spawn"));
    assert!(!names.contains(&"agent_observe"));
    assert!(!names.contains(&"agent_result"));

    let candidates = tools[0]["inputSchema"]["properties"]["policy"]["properties"]["candidates"]
        ["items"]["enum"]
        .as_array()
        .unwrap();
    assert!(candidates.contains(&json!("kimi")));
    assert!(!candidates.contains(&json!("pi")));
}
