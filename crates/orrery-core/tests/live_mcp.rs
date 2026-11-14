use orrery_core::config::{McpServerConfig, McpTransportConfig};
use orrery_core::mcp::{McpTransport, RmcpTransport, tool_result_is_error};
use orrery_core::security::InMemorySecretStore;
use serde_json::json;

#[tokio::test]
async fn live_stdio_list_and_call() {
    let raw_cmd = std::env::var("ORRERY_LIVE_MCP_CMD")
        .expect("ORRERY_LIVE_MCP_CMD must be set to a stdio MCP server command");
    let mut parts = raw_cmd.split_whitespace().map(String::from);
    let command = parts.next().expect("ORRERY_LIVE_MCP_CMD must not be empty");
    let args: Vec<String> = parts.collect();

    let server = McpServerConfig {
        server_id: "live".to_string(),
        transport: McpTransportConfig::Stdio { command, args },
        write_tools: Vec::new(),
    };
    let secrets = InMemorySecretStore::new();
    let transport = RmcpTransport::connect(&server, &secrets)
        .await
        .expect("RmcpTransport::connect should succeed");

    let tools = transport
        .list_tools()
        .await
        .expect("list_tools should succeed");
    assert!(!tools.is_empty(), "expected at least one MCP tool");

    let first_tool = &tools[0].name;
    let res = transport.call_tool(first_tool, json!({})).await;
    match res {
        Ok(val) => {
            let _is_err = tool_result_is_error(&val);
            assert!(val.is_object(), "tool call result should be a JSON object");
        }
        Err(err) => {
            panic!("unexpected transport error calling '{first_tool}': {err}");
        }
    }
}
