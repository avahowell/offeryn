use crate::McpServer;
use async_stream::stream;
use axum::{
    extract::{Json, Query},
    http::StatusCode,
    response::sse::{Event, Sse},
    routing::{get, post},
    Extension, Router,
};
use futures::stream::Stream;
use jsonrpc_core::{Call, Id, MethodCall, Output, Params, Request, Response, Version};
use std::convert::Infallible;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(serde::Deserialize)]
struct JsonRpcRequestWrapper {
    method: String,
    id: Option<serde_json::Value>,
    params: Option<serde_json::Value>,
}

pub struct SseTransport {
    connections: HashMap<String, mpsc::Sender<Result<Event, Infallible>>>,
}

impl SseTransport {
    pub fn new() -> Self {
        info!("Creating new SSE transport");
        Self {
            connections: HashMap::new(),
        }
    }

    pub fn create_router(server: Arc<McpServer>) -> Router {
        info!("Creating SSE router");
        let state = Arc::new(Mutex::new(Self::new()));

        Router::new()
            .route(
                "/sse",
                get(
                    |Extension(state): Extension<Arc<Mutex<SseTransport>>>| async move {
                        info!("New SSE connection request received");
                        Self::sse_handler(state).await
                    },
                ),
            )
            .route(
                "/message",
                post(
                    |Query(params): Query<HashMap<String, String>>,
                     Extension(state): Extension<Arc<Mutex<SseTransport>>>,
                     Extension(server): Extension<Arc<McpServer>>,
                     Json(request): Json<JsonRpcRequestWrapper>| async move {
                        let session_id = match params.get("sessionId") {
                            Some(id) => id,
                            None => {
                                error!("No sessionId provided in query parameters");
                                return Err(StatusCode::BAD_REQUEST);
                            }
                        };

                        info!(
                            session_id = %session_id,
                            "Received JSON-RPC request"
                        );

                        let params = match request.params {
                            Some(p) => match p.as_object() {
                                Some(obj) => Params::Map(obj.clone()),
                                None => Params::None,
                            },
                            None => Params::None,
                        };

                        let request = Request::Single(Call::MethodCall(MethodCall {
                            jsonrpc: Some(Version::V2),
                            method: request.method,
                            params,
                            id: request
                                .id
                                .map_or(Id::Null, |id| Id::Num(id.as_u64().unwrap_or(0))),
                        }));

                        Self::message_handler(session_id.clone(), state, server, request).await
                    },
                ),
            )
            .fallback(|req: axum::http::Request<axum::body::Body>| async move {
                error!(
                    method = %req.method(),
                    uri = %req.uri(),
                    "Request to unknown route"
                );
                StatusCode::NOT_FOUND
            })
            .layer(Extension(state))
            .layer(Extension(server))
    }

    async fn sse_handler(
        state: Arc<Mutex<SseTransport>>,
    ) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
        let (tx, mut rx) = mpsc::channel(100);
        let session_id = Uuid::new_v4().to_string();

        info!(
            session_id = %session_id,
            "New SSE connection established"
        );

        {
            let mut state = state.lock().unwrap();
            state.connections.insert(session_id.clone(), tx);
            info!(
                session_id = %session_id,
                active_connections = %state.connections.len(),
                "Added new SSE connection"
            );
        }

        let stream = stream! {
            info!(
                session_id = %session_id,
                "Sending endpoint URL"
            );
            // Send the endpoint URL with session ID
            let endpoint_url = format!("/message?sessionId={}", session_id);
            yield Ok(Event::default()
                .event("endpoint")
                .data(endpoint_url));

            info!(
                session_id = %session_id,
                "Starting event stream"
            );
            while let Some(event) = rx.recv().await {
                info!(
                    session_id = %session_id,
                    "Sending SSE event"
                );
                yield event;
            }
            info!(
                session_id = %session_id,
                "SSE connection closed"
            );
        };

        Sse::new(stream).keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(std::time::Duration::from_secs(1))
                .text("keep-alive-text"),
        )
    }

    async fn message_handler(
        session_id: String,
        state: Arc<Mutex<SseTransport>>,
        server: Arc<McpServer>,
        request: Request,
    ) -> Result<Json<Response>, StatusCode> {
        // Get the sender from the state
        let tx = {
            let state = state.lock().unwrap();
            if !state.connections.contains_key(&session_id) {
                warn!(
                    session_id = %session_id,
                    "Session ID not found"
                );
                return Err(StatusCode::NOT_FOUND);
            }
            info!(
                session_id = %session_id,
                "Found existing connection"
            );
            state.connections.get(&session_id).cloned().ok_or_else(|| {
                error!(
                    session_id = %session_id,
                    "Failed to get connection sender"
                );
                StatusCode::INTERNAL_SERVER_ERROR
            })?
        };

        // Process request with server
        let response = server.handle_request(request).await.map_err(|e| {
            error!(
                session_id = %session_id,
                error = %e,
                "Server request handler failed"
            );
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        // Send response through SSE channel if it's a successful response
        if let Response::Single(Output::Success(_)) = &response {
            // Ensure we send a proper JSON-RPC message
            let event =
                Event::default()
                    .event("message")
                    .data(serde_json::to_string(&response).map_err(|e| {
                        error!(
                            session_id = %session_id,
                            error = %e,
                            "Failed to serialize response"
                        );
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?);

            info!(
                session_id = %session_id,
                "Sending JSON-RPC response through SSE"
            );

            tx.send(Ok(event)).await.map_err(|e| {
                error!(
                    session_id = %session_id,
                    error = %e,
                    "Failed to send response through SSE channel"
                );
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        } else {
            info!(
                session_id = %session_id,
                "Skipping SSE for notification or error response"
            );
        }

        info!(
            session_id = %session_id,
            "Request completed successfully"
        );
        Ok(Json(response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::McpServer;
    use mcp_derive::mcp_tool;
    use serde_json::{json, Value};

    /// A simple calculator that can perform basic arithmetic operations
    #[derive(Default)]
    struct Calculator {}

    #[mcp_tool]
    impl Calculator {
        /// Add two numbers
        async fn add(&self, a: i64, b: i64) -> Result<i64, String> {
            Ok(a + b)
        }

        /// Subtract two numbers
        async fn subtract(&self, a: i64, b: i64) -> Result<i64, String> {
            Ok(a - b)
        }

        /// Multiply two numbers
        async fn multiply(&self, a: i64, b: i64) -> Result<i64, String> {
            Ok(a * b)
        }

        /// Divide two numbers
        async fn divide(&self, a: i64, b: i64) -> Result<f64, String> {
            if b == 0 {
                Err("Cannot divide by zero".to_string())
            } else {
                Ok(a as f64 / b as f64)
            }
        }
    }

    #[tokio::test]
    async fn test_calculator() {
        let server = Arc::new(McpServer::new("test-calculator", "1.0.0"));
        let calc = Calculator::default();

        // Register calculator tools
        server.register_tools(calc).await;

        // Test addition
        let request = Request::Single(Call::MethodCall(MethodCall {
            jsonrpc: Some(Version::V2),
            method: "tools/call".to_string(),
            params: Params::Map(
                json!({
                    "name": "calculator_add",
                    "arguments": {
                        "a": 2,
                        "b": 3
                    }
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
            id: Id::Num(1),
        }));

        let response = server.handle_request(request).await.unwrap();
        if let Response::Single(Output::Success(success)) = response {
            let result: Value = success.result;
            let content = result.get("content").unwrap().as_array().unwrap();
            let text = content[0].get("text").unwrap().as_str().unwrap();
            assert_eq!(text, "5"); // 2 + 3 = 5
        } else {
            panic!("Expected successful response");
        }

        // Test division
        let request = Request::Single(Call::MethodCall(MethodCall {
            jsonrpc: Some(Version::V2),
            method: "tools/call".to_string(),
            params: Params::Map(
                json!({
                    "name": "calculator_divide",
                    "arguments": {
                        "a": 10,
                        "b": 2
                    }
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
            id: Id::Num(2),
        }));

        let response = server.handle_request(request).await.unwrap();
        if let Response::Single(Output::Success(success)) = response {
            let result: Value = success.result;
            let content = result.get("content").unwrap().as_array().unwrap();
            let text = content[0].get("text").unwrap().as_str().unwrap();
            assert_eq!(text, "5"); // 10 / 2 = 5
        } else {
            panic!("Expected successful response");
        }

        // Test division by zero
        let request = Request::Single(Call::MethodCall(MethodCall {
            jsonrpc: Some(Version::V2),
            method: "tools/call".to_string(),
            params: Params::Map(
                json!({
                    "name": "calculator_divide",
                    "arguments": {
                        "a": 1,
                        "b": 0
                    }
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
            id: Id::Num(3),
        }));

        let response = server.handle_request(request).await.unwrap();
        if let Response::Single(Output::Success(success)) = response {
            let result: Value = success.result;
            let content = result.get("content").unwrap().as_array().unwrap();
            let text = content[0].get("text").unwrap().as_str().unwrap();
            let is_error = result.get("isError").unwrap().as_bool().unwrap();
            assert_eq!(text, "Cannot divide by zero");
            assert!(is_error);
        } else {
            panic!("Expected successful response");
        }
    }

    #[tokio::test]
    async fn test_sse_transport() {
        // Create a test server
        let server = McpServer::new("test-server", "1.0.0");
        let server = Arc::new(server);

        // Create the router
        let _app = SseTransport::create_router(server);
    }
}
