use async_trait;
use mcp_derive::mcp_tool;
use mcp_rs::{McpServer, StdioTransport};
use std::sync::Arc;

/// A simple calculator that can perform basic arithmetic operations
#[derive(Default, Clone)]
struct Calculator {}

#[mcp_tool]
impl Calculator {
    /// Add two numbers
    async fn add(&self, a: f64, b: f64) -> f64 {
        a + b
    }

    /// Subtract two numbers
    async fn subtract(&self, a: f64, b: f64) -> f64 {
        a - b
    }

    /// Multiply two numbers
    async fn multiply(&self, a: f64, b: f64) -> f64 {
        a * b
    }

    /// Divide two numbers
    async fn divide(&self, a: f64, b: f64) -> Result<f64, String> {
        if b == 0.0 {
            Err("Cannot divide by zero".to_string())
        } else {
            Ok(a / b)
        }
    }
}

#[tokio::main]
async fn main() {
    // Create a new server instance
    let server = Arc::new(McpServer::new("calculator", "1.0.0"));

    // Register the calculator tools
    server.register_tools(Calculator::default()).await;

    // Create and run the stdio transport
    let transport = StdioTransport::<tokio::io::Stdin, tokio::io::Stdout>::new(server);

    if let Err(e) = transport.run().await {
        eprintln!("Error: {}", e);
    }
}
