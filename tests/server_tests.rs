use mcp_server::{McpServer, JsonRpcRequest, McpTool, ToolContent, ToolResult};
use async_trait::async_trait;
use serde_json::{Value, json};

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
                r#type: "text".to_string(),
                text: echo.to_string(),
            }],
            is_error: false,
        })
    }
}

#[tokio::test]
async fn test_tools_list() {
    let mut server = McpServer::new("test-server".to_string(), "1.0.0".to_string());
    server.register_tool(Box::new(MockTool));

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "tools/list".to_string(),
        params: None,
    };

    let response = server.handle_request(request).await;

    assert!(response.error.is_none());
    assert!(response.result.is_some());

    let result = response.result.unwrap();
    let tools = result["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "mock_tool");
}

#[tokio::test]
async fn test_tool_execution() {
    let mut server = McpServer::new("test-server".to_string(), "1.0.0".to_string());
    server.register_tool(Box::new(MockTool));

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "mock_tool",
            "arguments": {
                "echo": "Hello, World!"
            }
        })),
    };

    let response = server.handle_request(request).await;

    assert!(response.error.is_none());
    assert!(response.result.is_some());

    let result = response.result.unwrap();
    assert_eq!(
        result["content"][0]["text"].as_str().unwrap(),
        "Hello, World!"
    );
    assert_eq!(result["isError"].as_bool().unwrap(), false);
}

#[tokio::test]
async fn test_unknown_tool() {
    let server = McpServer::new("test-server".to_string(), "1.0.0".to_string());

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "non_existent_tool",
            "arguments": {}
        })),
    };

    let response = server.handle_request(request).await;

    assert!(response.error.is_none());
    assert!(response.result.is_some());

    let result = response.result.unwrap();
    assert!(result["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Unknown tool"));
    assert!(result["isError"].as_bool().unwrap());
}

#[tokio::test]
async fn test_invalid_method() {
    let server = McpServer::new("test-server".to_string(), "1.0.0".to_string());

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "invalid/method".to_string(),
        params: None,
    };

    let response = server.handle_request(request).await;

    assert!(response.error.is_some());
    let error = response.error.unwrap();
    assert_eq!(error.code, -32601);
    assert_eq!(error.message, "Method not found");
}