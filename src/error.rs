use jsonrpc_core::{Error as JsonRpcError, ErrorCode};
use std::fmt;

#[derive(Debug)]
pub enum McpError {
    InvalidRequest,
    InvalidParams,
    MethodNotFound,
    InternalError,
}

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpError::InvalidRequest => write!(f, "Invalid request"),
            McpError::InvalidParams => write!(f, "Invalid parameters"),
            McpError::MethodNotFound => write!(f, "Method not found"),
            McpError::InternalError => write!(f, "Internal error"),
        }
    }
}

impl From<serde_json::Error> for McpError {
    fn from(_: serde_json::Error) -> Self {
        McpError::InternalError
    }
}

impl From<McpError> for JsonRpcError {
    fn from(error: McpError) -> Self {
        match error {
            McpError::InvalidRequest => JsonRpcError::invalid_request(),
            McpError::InvalidParams => JsonRpcError::invalid_params("Invalid parameters"),
            McpError::MethodNotFound => JsonRpcError::method_not_found(),
            McpError::InternalError => JsonRpcError::new(ErrorCode::ServerError(-32000)),
        }
    }
}
