#[tokio::main]
async fn main() {
    agent_bridge_mcp::runtime::main_entry().await;
}
