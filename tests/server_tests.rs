use async_trait::async_trait;
use jsonrpc_core::{Call, Id, MethodCall, Output, Params, Version};
use mcp_rs::{McpError, McpServer, McpTool};
use mcp_types::*;
use serde_json::{json, Value};
use std::sync::Arc;

// Mock tool for testing
struct MockTool;

#[async_trait]
impl McpTool for MockTool {
    fn name(&self) -> &str {
        "mock_tool"
    }

    fn description(&self) -> &str {
        "A mock tool for testing"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "echo": {
                    "type": "string"
                }
            }
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, String> {
        let echo = args["echo"].as_str().ok_or("Missing echo parameter")?;
        Ok(ToolResult {
            content: vec![ToolContent {
                text: echo.to_string(),
                r#type: "text".to_string(),
            }],
            is_error: false,
        })
    }
}

#[tokio::test]
async fn test_tools_list() {
    let server = Arc::new(McpServer::new("test-server", "1.0.0"));
    server.register_tool(MockTool).await;

    let request = JsonRpcRequest::Single(Call::MethodCall(MethodCall {
        jsonrpc: Some(Version::V2),
        id: Id::Num(1),
        method: "tools/list".to_string(),
        params: Params::None,
    }));

    let response = server.handle_request(request).await.unwrap();

    match response {
        JsonRpcResponse::Single(Output::Success(success)) => {
            let result: ListToolsResult = serde_json::from_value(success.result).unwrap();
            assert_eq!(result.tools.len(), 1);
            assert_eq!(result.tools[0].name, "mock_tool");
            assert_eq!(result.tools[0].description, "A mock tool for testing");
            assert!(result.next_page_token.is_none());
        }
        _ => panic!("Expected successful response"),
    }
}

#[tokio::test]
async fn test_tool_execution() {
    let server = Arc::new(McpServer::new("test-server", "1.0.0"));
    server.register_tool(MockTool).await;

    let params = serde_json::Map::from_iter(vec![
        ("name".to_string(), json!("mock_tool")),
        (
            "arguments".to_string(),
            json!({
                "echo": "Hello, World!"
            }),
        ),
    ]);

    let request = JsonRpcRequest::Single(Call::MethodCall(MethodCall {
        jsonrpc: Some(Version::V2),
        id: Id::Num(1),
        method: "tools/call".to_string(),
        params: Params::Map(params),
    }));

    let response = server.handle_request(request).await.unwrap();

    match response {
        JsonRpcResponse::Single(Output::Success(success)) => {
            let result: CallToolResult = serde_json::from_value(success.result).unwrap();
            assert_eq!(result.content.len(), 1);
            match &result.content[0] {
                Content::Text { text } => assert_eq!(text, "Hello, World!"),
                Content::Image { .. } => panic!("Expected text content"),
                Content::EmbeddedResource { .. } => panic!("Expected text content"),
            }
            assert_eq!(result.is_error, Some(false));
        }
        _ => panic!("Expected successful response"),
    }
}

#[tokio::test]
async fn test_unknown_tool() {
    let server = Arc::new(McpServer::new("test-server", "1.0.0"));

    let params = serde_json::Map::from_iter(vec![
        ("name".to_string(), json!("non_existent_tool")),
        ("arguments".to_string(), json!({})),
    ]);

    let request = JsonRpcRequest::Single(Call::MethodCall(MethodCall {
        jsonrpc: Some(Version::V2),
        id: Id::Num(1),
        method: "tools/call".to_string(),
        params: Params::Map(params),
    }));

    let response = server.handle_request(request).await;
    assert!(matches!(response, Err(McpError::MethodNotFound)));
}

#[tokio::test]
async fn test_invalid_method() {
    let server = Arc::new(McpServer::new("test-server", "1.0.0"));

    let request = JsonRpcRequest::Single(Call::MethodCall(MethodCall {
        jsonrpc: Some(Version::V2),
        id: Id::Num(1),
        method: "invalid/method".to_string(),
        params: Params::None,
    }));

    let response = server.handle_request(request).await.unwrap();

    match response {
        JsonRpcResponse::Single(Output::Failure(failure)) => {
            assert_eq!(
                failure.error.code.code(),
                jsonrpc_core::ErrorCode::MethodNotFound.code()
            );
            assert_eq!(failure.error.message, "Method not found");
        }
        _ => panic!("Expected failure response"),
    }
}

#[tokio::test]
async fn test_initialize() {
    let server = Arc::new(McpServer::new("test-server", "1.0.0"));
    server.register_tool(MockTool).await;

    let request = JsonRpcRequest::Single(Call::MethodCall(MethodCall {
        jsonrpc: Some(Version::V2),
        id: Id::Num(1),
        method: "initialize".to_string(),
        params: Params::None,
    }));

    let response = server.handle_request(request).await.unwrap();

    match response {
        JsonRpcResponse::Single(Output::Success(success)) => {
            let result: InitializeResult = serde_json::from_value(success.result).unwrap();
            assert_eq!(result.server_info.name, "test-server");
            assert_eq!(result.server_info.version, "1.0.0");
            assert_eq!(result.protocol_version, LATEST_PROTOCOL_VERSION);
            assert_eq!(result.capabilities.tools.len(), 1);
            assert!(result.capabilities.tools.contains_key("mock_tool"));
        }
        _ => panic!("Expected successful response"),
    }
}
