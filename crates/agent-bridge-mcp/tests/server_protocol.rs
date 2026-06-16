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

fn assert_before(text: &str, first: &str, second: &str) {
    let first_index = text
        .find(first)
        .unwrap_or_else(|| panic!("missing first marker: {first}"));
    let second_index = text
        .find(second)
        .unwrap_or_else(|| panic!("missing second marker: {second}"));
    assert!(
        first_index < second_index,
        "expected {first:?} before {second:?} in {text:?}"
    );
}

const PUBLIC_TOOLS: [&str; 8] = [
    "providers_list",
    "doctor",
    "agent_spawn",
    "agent_list",
    "agent_observe",
    "agent_result",
    "agent_stop",
    "agent_remove",
];

const REMOVED_TOOLS: [&str; 6] = [
    "providers_check",
    "agent_preview",
    "agent_status",
    "agent_wait",
    "agent_logs",
    "agent_transcript",
];

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
    let instructions = result["instructions"].as_str().unwrap();
    assert!(instructions.contains("Provider output is evidence only"));
    assert!(
        instructions[..512.min(instructions.len())]
            .contains("caller still owns project verification")
    );
    assert!(instructions.contains("Eight-tool workflow"));
    assert_before(instructions, "agent_spawn", "agent_observe");
    assert_before(instructions, "agent_observe", "agent_result");
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
            "agent_bridge_recover_stalled_task",
            "agent_bridge_claude_host_lifecycle",
            "agent_bridge_dogfood_workflows",
            "agent_bridge_compare_providers"
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

    assert!(text.contains("agent_spawn"));
    assert!(text.contains("agent_observe"));
    assert!(text.contains("agent_result"));
    assert_before(text, "agent_spawn", "agent_observe");
    assert!(text.contains("main caller remains responsible"));
    for removed in REMOVED_TOOLS {
        assert!(!text.contains(removed), "prompt should not name {removed}");
    }

    let response = handle_request(request(
        "prompts/get",
        9,
        serde_json::json!({ "name": "agent_bridge_claude_host_lifecycle" }),
    ))
    .await;
    let result = response.unwrap().result.unwrap();
    let text = result["messages"][0]["content"]["text"].as_str().unwrap();

    assert!(text.contains("claude-host-runner"));
    assert!(text.contains("ping"));
    assert!(text.contains("doctor"));
    assert!(text.contains("workspace_policy_mismatch"));
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
            "agent-bridge://guidance/provider-capabilities",
            "agent-bridge://guidance/claude-host-lifecycle",
            "agent-bridge://guidance/dogfood-workflows",
            "agent-bridge://guidance/code-execution"
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
    let text = content["text"].as_str().unwrap();
    assert!(text.contains("doctor"));
    assert!(text.contains("reviewPacket"));
    assert!(text.contains("agent_remove"));
    assert!(text.contains("Primary flow"));
    assert_before(text, "agent_spawn", "agent_observe");
    assert_before(text, "agent_observe", "agent_result");
    for removed in REMOVED_TOOLS {
        assert!(
            !text.contains(removed),
            "caller workflow should not name {removed}"
        );
    }

    let response = handle_request(request(
        "resources/read",
        11,
        serde_json::json!({ "uri": "agent-bridge://guidance/claude-host-lifecycle" }),
    ))
    .await;
    let result = response.unwrap().result.unwrap();
    let text = result["contents"][0]["text"].as_str().unwrap();
    assert!(text.contains("doctor"));
    assert!(text.contains("workspace_policy_mismatch"));

    let response = handle_request(request(
        "resources/read",
        10,
        serde_json::json!({ "uri": "agent-bridge://guidance/dogfood-workflows" }),
    ))
    .await;
    let result = response.unwrap().result.unwrap();
    let text = result["contents"][0]["text"].as_str().unwrap();
    assert!(text.contains("read-only review"));
    assert!(text.contains("isolated implementation"));
    assert!(text.contains("provider comparison"));
    assert!(text.contains("Use `agent_observe` as the primary progress path"));
}

#[tokio::test]
async fn code_execution_guidance_resource_is_available() {
    let response = handle_request(request(
        "resources/read",
        20,
        serde_json::json!({ "uri": "agent-bridge://guidance/code-execution" }),
    ))
    .await;
    let result = response.unwrap().result.unwrap();
    let text = result["contents"][0]["text"].as_str().unwrap();
    assert!(text.contains("agent_observe"));
    assert!(text.contains("sections"));
    assert!(text.contains("out of context"));
    assert!(text.contains("verification"));
}

fn assert_codex_denial_guidance(text: &str, surface: &str) {
    let mentions_symptom = [
        "patch rejected",
        "sandbox denial",
        "approval denial",
        "outside of the project",
        "out-of-workspace",
    ]
    .iter()
    .any(|symptom| text.contains(symptom));
    assert!(
        mentions_symptom,
        "{surface} should mention Codex denial symptoms"
    );

    for tool in ["agent_observe", "agent_result"] {
        assert!(text.contains(tool), "{surface} should mention {tool}");
    }

    for inspection in ["cwd", "workspace policy", "prompt scope", "isolation"] {
        assert!(
            text.contains(inspection),
            "{surface} should tell callers to inspect {inspection}"
        );
    }

    let lower = text.to_ascii_lowercase();
    assert!(
        !lower.contains("silently relax sandbox") && !lower.contains("blindly retry"),
        "{surface} should warn against unsafe retry instead of recommending it"
    );
}

#[tokio::test]
async fn codex_denial_guidance_is_documented_in_recovery_safety_and_provider_surfaces() {
    let recover = handle_request(request(
        "prompts/get",
        12,
        serde_json::json!({ "name": "agent_bridge_recover_stalled_task" }),
    ))
    .await
    .unwrap()
    .result
    .unwrap();
    assert_codex_denial_guidance(
        recover["messages"][0]["content"]["text"].as_str().unwrap(),
        "recover stalled prompt",
    );

    let safety = handle_request(request(
        "resources/read",
        13,
        serde_json::json!({ "uri": "agent-bridge://guidance/safety" }),
    ))
    .await
    .unwrap()
    .result
    .unwrap();
    assert_codex_denial_guidance(
        safety["contents"][0]["text"].as_str().unwrap(),
        "safety resource",
    );

    let providers = handle_request(request(
        "resources/read",
        14,
        serde_json::json!({ "uri": "agent-bridge://guidance/provider-capabilities" }),
    ))
    .await
    .unwrap()
    .result
    .unwrap();
    assert_codex_denial_guidance(
        providers["contents"][0]["text"].as_str().unwrap(),
        "provider capabilities resource",
    );
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
async fn tools_list_returns_consolidated_public_tool_names() {
    let response = handle_request(request("tools/list", 8, serde_json::json!({}))).await;
    let result = response.unwrap().result.unwrap();
    let names: Vec<_> = result["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();

    assert_eq!(names, PUBLIC_TOOLS.to_vec());
    for removed in REMOVED_TOOLS {
        assert!(
            !names.contains(&removed),
            "removed tool should not be listed: {removed}"
        );
    }
    assert!(
        names.iter().all(|name| !name.starts_with("task_")),
        "legacy task_* lifecycle tools should not be listed: {names:?}"
    );
}

#[tokio::test]
async fn tools_carry_read_only_and_destructive_annotations() {
    let response = handle_request(request("tools/list", 17, serde_json::json!({}))).await;
    let result = response.unwrap().result.unwrap();
    let tools = result["tools"].as_array().unwrap();
    let annotations = |name: &str| {
        tools
            .iter()
            .find(|tool| tool["name"] == name)
            .map(|tool| &tool["annotations"])
            .unwrap_or_else(|| panic!("missing tool {name}"))
    };

    for name in [
        "providers_list",
        "doctor",
        "agent_observe",
        "agent_result",
        "agent_list",
    ] {
        assert_eq!(annotations(name)["readOnlyHint"], true, "{name}");
    }
    assert_eq!(annotations("agent_remove")["destructiveHint"], true);
    assert_eq!(annotations("agent_stop")["destructiveHint"], true);
    assert_eq!(annotations("agent_spawn")["readOnlyHint"], false);
}

#[tokio::test]
async fn tool_descriptions_describe_consolidated_surface() {
    let response = handle_request(request("tools/list", 18, serde_json::json!({}))).await;
    let result = response.unwrap().result.unwrap();
    let tools = result["tools"].as_array().unwrap();
    let description = |name: &str| {
        tools
            .iter()
            .find(|tool| tool["name"] == name)
            .and_then(|tool| tool["description"].as_str())
            .unwrap_or_else(|| panic!("missing description for {name}"))
    };

    assert!(description("agent_spawn").contains("Primary follow-ups"));
    assert!(description("agent_spawn").contains("dryRun"));
    assert!(description("agent_observe").contains("Primary progress path"));
    assert!(description("agent_observe").contains("until"));
    assert!(description("agent_result").contains("Primary final evidence path"));
    assert!(description("agent_result").contains("sections"));
    assert!(description("doctor").contains("readiness-only"));
}

#[tokio::test]
async fn public_tool_input_schemas_remain_strict_and_compatible() {
    let response = handle_request(request("tools/list", 19, serde_json::json!({}))).await;
    let result = response.unwrap().result.unwrap();
    let tools = result["tools"].as_array().unwrap();
    let tool = |name: &str| {
        tools
            .iter()
            .find(|tool| tool["name"] == name)
            .unwrap_or_else(|| panic!("missing tool {name}"))
    };
    let schema = |name: &str| &tool(name)["inputSchema"];

    let empty_required = serde_json::json!([]);
    for name in ["providers_list", "doctor", "agent_list"] {
        assert_eq!(schema(name)["type"], "object", "{name}");
        assert_eq!(schema(name)["additionalProperties"], false, "{name}");
        assert_eq!(schema(name)["required"], empty_required, "{name}");
    }

    assert_eq!(schema("agent_spawn")["type"], "object");
    assert_eq!(schema("agent_spawn")["additionalProperties"], false);
    assert_eq!(
        schema("agent_spawn")["required"],
        serde_json::json!(["provider", "mode", "prompt"])
    );
    for property in [
        "provider",
        "mode",
        "prompt",
        "title",
        "cwd",
        "timeoutSeconds",
        "model",
        "effort",
        "thinking",
        "isolation",
        "worktreeName",
        "profile",
        "dryRun",
    ] {
        assert!(
            schema("agent_spawn")["properties"].get(property).is_some(),
            "agent_spawn missing {property}"
        );
    }

    for name in ["agent_stop", "agent_remove"] {
        assert_eq!(schema(name)["additionalProperties"], false, "{name}");
        assert_eq!(
            schema(name)["required"],
            serde_json::json!(["agentId"]),
            "{name}"
        );
        assert!(schema(name)["properties"].get("taskId").is_none(), "{name}");
    }

    for name in ["agent_observe", "agent_result"] {
        assert_eq!(schema(name)["type"], "object", "{name}");
        assert_eq!(schema(name)["additionalProperties"], false, "{name}");
        assert_eq!(
            schema(name)["required"],
            serde_json::json!(["agentId"]),
            "{name}"
        );
        assert!(
            schema(name)["properties"].get("agentId").is_some(),
            "{name}"
        );
        assert!(schema(name)["properties"].get("taskId").is_none(), "{name}");
        assert!(
            schema(name)["properties"].get("verbosity").is_some(),
            "{name} should expose verbosity"
        );
    }

    // Subsuming parameters are advertised.
    assert!(schema("agent_spawn")["properties"].get("dryRun").is_some());
    assert!(schema("agent_observe")["properties"].get("until").is_some());
    assert!(
        schema("agent_result")["properties"]
            .get("sections")
            .is_some()
    );
    assert!(schema("doctor")["properties"].get("focus").is_some());
    assert!(schema("doctor")["properties"].get("cwd").is_some());
    assert!(schema("doctor")["properties"].get("smoke").is_some());
    assert_eq!(
        schema("agent_spawn")["properties"]["profile"]["enum"],
        serde_json::json!(["bridge", "bare", "unblocked"])
    );
    assert_eq!(schema("agent_list")["properties"]["limit"]["maximum"], 100);
    assert_eq!(
        schema("agent_observe")["properties"]["limit"]["maximum"],
        500
    );
}

#[tokio::test]
async fn doctor_is_listed_with_strict_schema_and_rejects_unknown_arguments() {
    let response = handle_request(request("tools/list", 9, serde_json::json!({}))).await;
    let result = response.unwrap().result.unwrap();
    let tools = result["tools"].as_array().unwrap();
    let doctor = tools
        .iter()
        .find(|tool| tool["name"] == "doctor")
        .expect("doctor tool should be listed");

    assert_eq!(doctor["inputSchema"]["additionalProperties"], false);
    assert_eq!(doctor["inputSchema"]["required"], serde_json::json!([]));
    assert_eq!(
        doctor["inputSchema"]["properties"]["focus"]["enum"],
        serde_json::json!(["all", "providers"])
    );
    assert_eq!(
        doctor["inputSchema"]["properties"]["smoke"]["type"],
        "boolean"
    );
    assert_eq!(
        doctor["inputSchema"]["properties"]["providers"]["items"]["enum"],
        serde_json::json!(["claude", "cursor", "kimi", "codex", "forge", "antigravity"])
    );
    assert_eq!(
        doctor["inputSchema"]["properties"]["aggregateTimeoutMs"]["maximum"],
        120000
    );
    assert_eq!(
        doctor["inputSchema"]["properties"]["providerTimeoutMs"]["additionalProperties"]["maximum"],
        90000
    );
    assert_eq!(
        doctor["inputSchema"]["properties"]["profile"]["enum"],
        serde_json::json!(["bridge", "bare", "unblocked"])
    );
    assert_eq!(
        doctor["outputSchema"]["properties"]["launchReadiness"]["type"],
        "object"
    );
    assert_eq!(
        doctor["outputSchema"]["properties"]["recommendations"]["type"],
        "array"
    );

    let response = handle_request(request(
        "tools/call",
        10,
        serde_json::json!({
            "name": "doctor",
            "arguments": { "smoke": true, "maxTurns": 2 }
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
            .contains("Unknown argument for doctor: maxTurns")
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
    assert_eq!(result["structuredContent"], payload);
    assert_eq!(
        payload["providers"]["codex"]["supportsWorktreeIsolation"],
        true
    );
    assert_eq!(
        payload["providers"]["codex"]["launchProfiles"],
        serde_json::json!(["bridge", "bare"])
    );
    assert_eq!(
        payload["providers"]["claude"]["launchProfiles"],
        serde_json::json!(["bridge", "bare", "unblocked"])
    );
    assert_eq!(
        payload["providers"]["cursor"]["outputCadence"]["cadence"],
        "final_json"
    );
    assert_eq!(payload["providers"]["codex"]["readiness"]["state"], "stale");
    assert_eq!(
        payload["providers"]["forge"]["launchProfiles"],
        serde_json::json!(["bridge", "bare"])
    );
    assert_eq!(
        payload["providers"]["forge"]["reducedConfiguration"]["configIsolation"],
        "best_effort"
    );
    assert_eq!(
        payload["providers"]["antigravity"]["launchProfiles"],
        serde_json::json!(["bridge", "bare", "unblocked"])
    );
    assert_eq!(
        payload["providers"]["antigravity"]["readOnlyEnforcement"]["review"],
        "prompt_enforced"
    );
}

#[tokio::test]
async fn consolidated_agent_read_schemas_expose_lean_next_list() {
    let response = handle_request(request("tools/list", 15, serde_json::json!({}))).await;
    let result = response.unwrap().result.unwrap();
    let tools = result["tools"].as_array().unwrap();

    for tool_name in ["agent_observe", "agent_result"] {
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == tool_name)
            .expect("tool should be listed");
        assert_eq!(
            tool["outputSchema"]["properties"]["next"]["type"], "array",
            "{tool_name}"
        );
        assert!(
            tool["outputSchema"]["properties"]
                .get("nextActions")
                .is_none(),
            "{tool_name} should not expose duplicated nextActions"
        );
        assert!(
            tool["outputSchema"]["properties"]
                .get("presentation")
                .is_none(),
            "{tool_name} should not expose GUI presentation"
        );
        assert!(
            tool["inputSchema"]["properties"].get("agentId").is_some(),
            "{tool_name}"
        );
        assert!(
            tool["outputSchema"]["properties"].get("agentId").is_some(),
            "{tool_name}"
        );
        assert!(
            tool["outputSchema"]["properties"].get("taskId").is_none(),
            "{tool_name}"
        );
    }
}

#[tokio::test]
async fn agent_list_schema_exposes_bounded_filters() {
    let response = handle_request(request("tools/list", 16, serde_json::json!({}))).await;
    let result = response.unwrap().result.unwrap();
    let tools = result["tools"].as_array().unwrap();
    let agent_list = tools
        .iter()
        .find(|tool| tool["name"] == "agent_list")
        .expect("agent_list tool should be listed");
    let properties = &agent_list["inputSchema"]["properties"];

    assert_eq!(agent_list["inputSchema"]["additionalProperties"], false);
    assert_eq!(agent_list["inputSchema"]["required"], serde_json::json!([]));
    assert_eq!(
        agent_list["outputSchema"]["properties"]["agents"]["type"],
        "array"
    );
    assert!(properties.get("presentation").is_none());
    assert!(properties.get("scope").is_none());
    assert_eq!(
        properties["status"]["items"]["enum"],
        serde_json::json!([
            "queued",
            "running",
            "succeeded",
            "failed",
            "stopped",
            "failed_stale",
            "removed"
        ])
    );
    assert_eq!(properties["limit"]["maximum"], 100);
}

#[tokio::test]
async fn agent_spawn_rejects_unknown_public_fields() {
    let response = handle_request(request(
        "tools/call",
        5,
        serde_json::json!({
            "name": "agent_spawn",
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
