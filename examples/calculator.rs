use mcp_derive::mcp_tool;
use mcp_rs::{McpServer, SseTransport};
use std::sync::Arc;
use tokio;
use axum;

/// A simple calculator that can perform basic arithmetic operations
#[mcp_tool]
trait Calculator {
    /// Add two numbers
    async fn add(&self, a: i64, b: i64) -> i64 {
        a + b
    }

    /// Subtract two numbers
    async fn subtract(&self, a: i64, b: i64) -> i64 {
        a - b
    }

    /// Multiply two numbers
    async fn multiply(&self, a: i64, b: i64) -> i64 {
        a * b
    }

    /// Divide two numbers
    async fn divide(&self, a: i64, b: i64) -> Result<f64, &'static str> {
        if b == 0 {
            Err("Cannot divide by zero")
        } else {
            Ok(a as f64 / b as f64)
        }
    }
}

#[derive(Default, Clone)]
struct CalculatorImpl;
impl Calculator for CalculatorImpl {}

#[tokio::main]
async fn main() {
    // Create and configure server
    let mut server = McpServer::new();

    // Register the calculator tools
    server.register_tools(CalculatorImpl::default());

    // Create shared server instance
    let server = Arc::new(server);

    // Create router with SSE transport
    let app = SseTransport::create_router(server);

    println!("MCP Server starting on http://0.0.0.0:3000");
    println!("Try calling the calculator tools with:");
    println!(r#"# First get a connection ID:
curl http://localhost:3000/mcp/events

# Then use it to make calls:
curl -X POST http://localhost:3000/mcp/$CONNECTION_ID \
    -H "Content-Type: application/json" \
    -d '{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"calculator_add","arguments":{{"a":2,"b":3}}}}}}'

# Try other operations:
curl -X POST http://localhost:3000/mcp/$CONNECTION_ID \
    -H "Content-Type: application/json" \
    -d '{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"calculator_multiply","arguments":{{"a":4,"b":5}}}}}}'
"#);
    
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
} 