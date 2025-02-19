use async_trait::async_trait;
pub use jsonrpc_core::{
    Error as JsonRpcError, Id, Params, Request as JsonRpcRequest, Response as JsonRpcResponse,
    Version,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolContent {
    pub r#type: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: Vec<ToolContent>,
    pub is_error: bool,
}

#[async_trait]
pub trait McpTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    async fn execute(&self, args: Value) -> Result<ToolResult, String>;
}

pub trait HasTools {
    type Tools: IntoIterator<Item = Box<dyn McpTool>>;
    fn tools(self) -> Self::Tools;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Implementation {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    pub experimental: HashMap<String, Value>,
    pub sampling: HashMap<String, Value>,
    pub roots: RootsCapability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootsCapability {
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    pub client_info: Implementation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    pub tools: HashMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    /// The version of the Model Context Protocol that the server wants to use.
    /// This may not match the version that the client requested.
    /// If the client cannot support this version, it MUST disconnect.
    pub protocol_version: String,

    /// The server's capabilities
    pub capabilities: ServerCapabilities,

    /// Information about the server implementation
    pub server_info: ServerInfo,

    /// Optional instructions for using the server
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

pub const LATEST_PROTOCOL_VERSION: &str = "2024-11-05";
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2024-11-05"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonRpcMessage<T> {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(flatten)]
    pub content: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeRequest {
    pub method: String, // Will be "initialize"
    pub params: InitializeParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResponse {
    pub result: InitializeResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializedNotification {
    pub params: InitializedNotificationParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializedNotificationParams {
    pub meta: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsResult {
    /// Array of available tools
    pub tools: Vec<Tool>,

    /// Optional pagination token for getting next page of results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    /// Name of the tool
    pub name: String,

    /// Description of what the tool does
    pub description: String,

    /// JSON Schema describing the tool's input parameters
    pub input_schema: Value, // Using serde_json::Value for the JSON Schema object
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallToolRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<std::collections::HashMap<String, Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    pub content: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Content {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "image")]
    Image {
        url: String,
        mime_type: Option<String>,
    },

    #[serde(rename = "resource")]
    EmbeddedResource { uri: String, name: Option<String> },
}

// Collin: Pulled this over from the types generated from the MCP spec schema.
// There are missing variants for now.
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum ServerResult {
    InitializeResult(InitializeResult),
}

impl From<&ServerResult> for ServerResult {
    fn from(value: &ServerResult) -> Self {
        value.clone()
    }
}

impl From<InitializeResult> for ServerResult {
    fn from(value: InitializeResult) -> Self {
        Self::InitializeResult(value)
    }
}
