use orrery_core::config::{McpServerConfig, McpTransportConfig};
use orrery_core::mcp::{McpTransport, RmcpTransport, tool_result_is_error, tool_result_text};
use orrery_core::security::InMemorySecretStore;
use serde_json::json;

fn live_server() -> McpServerConfig {
    let Ok(raw_cmd) = std::env::var("ORRERY_LIVE_MCP_CMD") else {
        panic!("ORRERY_LIVE_MCP_CMD must be set to a stdio MCP server command");
    };
    let mut parts = raw_cmd.split_whitespace().map(String::from);
    let Some(command) = parts.next() else {
        panic!("ORRERY_LIVE_MCP_CMD must not be empty");
    };
    let args: Vec<String> = parts.collect();

    McpServerConfig {
        server_id: "live".to_string(),
        transport: McpTransportConfig::Stdio { command, args },
        write_tools: Vec::new(),
    }
}

#[tokio::test]
async fn live_stdio_list_and_call() {
    let server = live_server();
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

#[tokio::test]
async fn live_stdio_call_with_merged_args() {
    let tool = std::env::var("ORRERY_LIVE_MCP_TOOL")
        .expect("ORRERY_LIVE_MCP_TOOL must name a tool on the ORRERY_LIVE_MCP_CMD server");
    let raw = std::env::var("ORRERY_LIVE_MCP_ARGS").unwrap_or_else(|_| "{}".to_string());
    let configured = serde_json::from_str::<serde_json::Value>(&raw)
        .expect("ORRERY_LIVE_MCP_ARGS must be JSON")
        .as_object()
        .cloned()
        .expect("ORRERY_LIVE_MCP_ARGS must be a JSON object");
    let orrery = json!({
        "project_name": "live-test",
        "run_id": "00000000-0000-0000-0000-000000000000",
        "step_number": 1,
        "step_title": "live merged args",
        "plan_step_sha256": "0".repeat(64),
        "event": "step_pass"
    });
    let args = orrery_core::engine::merge_tool_args(&configured, orrery);
    let transport = RmcpTransport::connect(&live_server(), &InMemorySecretStore::new())
        .await
        .expect("RmcpTransport::connect should succeed");
    let val = transport
        .call_tool(&tool, args)
        .await
        .unwrap_or_else(|err| panic!("transport error calling '{tool}': {err}"));
    assert!(
        !tool_result_is_error(&val),
        "'{tool}' returned isError: {}",
        tool_result_text(&val)
    );
}
