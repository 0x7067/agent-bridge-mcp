use serde_json::Value;
use std::{fs, path::PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("crate lives under crates/agent-bridge-mcp")
        .to_path_buf()
}

#[test]
fn cursor_project_mcp_config_registers_agent_bridge() {
    let path = repo_root().join(".cursor/mcp.json");
    let contents = fs::read_to_string(&path).expect(".cursor/mcp.json should be checked in");
    let config: Value = serde_json::from_str(&contents).expect(".cursor/mcp.json should be JSON");
    let server = &config["mcpServers"]["agent-bridge"];

    assert_eq!(server["command"], "agent-bridge-mcp");
    assert_eq!(server["args"], Value::Array(Vec::new()));
    assert_eq!(
        server["env"]["AGENT_BRIDGE_WORKSPACES"],
        "${workspaceFolder}"
    );
    assert_eq!(
        server["env"]["AGENT_BRIDGE_STATE_DIR"],
        "${env:HOME}/.agent-bridge-mcp/state"
    );
    assert!(
        server["env"]
            .get("AGENT_BRIDGE_CLAUDE_HOST_SOCKET")
            .is_none(),
        "repo-level Cursor config should not require Claude host runner setup"
    );
}
