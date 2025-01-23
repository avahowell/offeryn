use crate::McpError;
use mcp_types::*;
use std::collections::HashMap;
use tracing::{info, debug, warn};
use jsonrpc_core::{Params, Success, Output, Failure, Version, Call, ErrorCode};

pub struct McpServer {
    name: String,
    version: String,
    tools: HashMap<String, Box<dyn McpTool>>,
}

impl McpServer {
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            tools: HashMap::new(),
        }
    }

    pub fn with_tool(&mut self, tool: impl McpTool + 'static) -> &mut Self {
        let tool_name = tool.name().to_string();
        info!(tool_name = %tool_name, "Registering tool");
        self.tools.insert(tool_name, Box::new(tool));
        self
    }

    pub fn with_tools(&mut self, tools: Vec<Box<dyn McpTool>>) -> &mut Self {
        for tool in tools {
            let name = tool.name().to_string();
            info!(tool_name = %name, "Registering tool");
            self.tools.insert(name, tool);
        }
        self
    }

    pub fn register_tool<T: McpTool + 'static>(&mut self, tool: T) {
        let tool_name = tool.name().to_string();
        info!(tool_name = %tool_name, "Registering tool");
        self.tools.insert(tool_name, Box::new(tool));
    }

    pub fn register_tools<T: HasTools>(&mut self, provider: T)
    where
        T::Tools: IntoIterator<Item = Box<dyn McpTool>>
    {
        for tool in provider.tools() {
            let name = tool.name().to_string();
            info!(tool_name = %name, "Registering tool");
            self.tools.insert(name, tool);
        }
    }

    pub async fn handle_request(&mut self, request: JsonRpcRequest) -> Result<JsonRpcResponse, McpError> {
        let (id, method, params) = match request {
            JsonRpcRequest::Single(Call::MethodCall(call)) => {
                debug!(
                    method = %call.method,
                    id = ?call.id,
                    params = %serde_json::to_string_pretty(&call.params).unwrap_or_default(),
                    "Received JSON-RPC request"
                );
                (call.id, call.method, call.params)
            },
            _ => {
                warn!("Invalid request format - expected MethodCall");
                return Err(McpError::InvalidRequest);
            }
        };

        let response = match method.as_str() {
            "initialize" => {
                info!("Processing initialize request");
                let capabilities = ServerCapabilities {
                    tools: self.tools.keys().map(|k| (k.clone(), true)).collect(),
                };

                let result = InitializeResult {
                    protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
                    capabilities,
                    server_info: ServerInfo {
                        name: self.name.clone(),
                        version: self.version.clone(),
                    },
                    instructions: Some("Use tools/list to see available tools".to_string()),
                };

                debug!(
                    server_name = %self.name,
                    server_version = %self.version,
                    protocol_version = %LATEST_PROTOCOL_VERSION,
                    num_tools = %self.tools.len(),
                    "Sending initialize response"
                );

                JsonRpcResponse::Single(Output::Success(Success {
                    jsonrpc: Some(Version::V2),
                    result: serde_json::to_value(result)?,
                    id,
                }))
            }
            "tools/list" => {
                info!("Processing tools/list request");
                let tools: Vec<Tool> = self.tools.values()
                    .map(|tool| Tool {
                        name: tool.name().to_string(),
                        description: tool.description().to_string(),
                        input_schema: tool.input_schema(),
                    })
                    .collect();

                let result = ListToolsResult {
                    tools,
                    next_page_token: None, // Pagination not implemented yet
                };

                debug!(
                    num_tools = %result.tools.len(),
                    tool_names = ?result.tools.iter().map(|t| &t.name).collect::<Vec<_>>(),
                    "Sending tools list response"
                );

                JsonRpcResponse::Single(Output::Success(Success {
                    jsonrpc: Some(Version::V2),
                    result: serde_json::to_value(result)?,
                    id,
                }))
            }
            "tools/call" => {
                info!("Processing tools/call request");
                let params = match params {
                    Params::Map(map) => map,
                    _ => {
                        warn!("Invalid params format for tools/call - expected Map");
                        return Err(McpError::InvalidParams);
                    }
                };

                let request: CallToolRequest = serde_json::from_value(serde_json::Value::Object(params))
                    .map_err(|_| {
                        warn!("Failed to parse tool call request parameters");
                        McpError::InvalidParams
                    })?;

                debug!(
                    tool = %request.name,
                    args = ?request.arguments,
                    "Executing tool"
                );

                let tool = self.tools.get(&request.name)
                    .ok_or_else(|| {
                        warn!(tool = %request.name, "Tool not found");
                        McpError::MethodNotFound
                    })?;

                let args = match request.arguments {
                    Some(args) => serde_json::Value::Object(args.into_iter().collect()),
                    None => serde_json::json!({})
                };

                debug!(
                    tool = %request.name,
                    args = %serde_json::to_string_pretty(&args).unwrap_or_default(),
                    "Executing tool with arguments"
                );

                match tool.execute(args).await {
                    Ok(result) => {
                        let content = result.content.into_iter()
                            .map(|c| Content::Text { text: c.text })
                            .collect();

                        let result = CallToolResult {
                            content,
                            is_error: Some(result.is_error),
                        };

                        debug!(
                            tool = %request.name,
                            is_error = ?result.is_error,
                            content_length = %result.content.len(),
                            "Tool execution successful"
                        );

                        JsonRpcResponse::Single(Output::Success(Success {
                            jsonrpc: Some(Version::V2),
                            result: serde_json::to_value(result)?,
                            id,
                        }))
                    },
                    Err(e) => {
                        warn!(
                            tool = %request.name,
                            error = %e,
                            "Tool execution failed"
                        );
                        JsonRpcResponse::Single(Output::Failure(Failure {
                            jsonrpc: Some(Version::V2),
                            error: JsonRpcError::new(ErrorCode::ServerError(-32000)),
                            id,
                        }))
                    },
                }
            }
            _ => {
                warn!(method = %method, "Unknown method called");
                JsonRpcResponse::Single(Output::Failure(Failure {
                    jsonrpc: Some(Version::V2),
                    error: JsonRpcError::method_not_found(),
                    id,
                }))
            }
        };

        // Log the full JSON response
        info!(
            method = %method,
            response = %serde_json::to_string_pretty(&response).unwrap_or_default(),
            "Full JSON response"
        );

        Ok(response)
    }

    pub fn handle_notification(&mut self, method: &str, _params: Option<serde_json::Value>) -> Result<(), McpError> {
        match method {
            "notifications/initialized" => {
                info!("Client completed initialization");
                Ok(())
            }
            _ => Err(McpError::MethodNotFound)
        }
    }
}