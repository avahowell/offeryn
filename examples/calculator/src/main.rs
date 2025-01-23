use mcp_derive::mcp_tool;
use mcp_rs::{McpServer, SseTransport};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing_subscriber::{EnvFilter};
use std::net::SocketAddr;

/// A simple calculator that can perform basic arithmetic operations
#[mcp_tool]
#[async_trait::async_trait]
trait Calculator {
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

#[tokio::main]
async fn main() {
    // Initialize logging with all levels enabled
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("debug,mcp_rs=debug,tower_http=debug"))
        )
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    // Create a new server instance
    let mut server = McpServer::new("calculator", "1.0.0");
    
    // Register the calculator tools
    server.register_tools(CalculatorImpl::default());
    
    let server = Arc::new(Mutex::new(server));

    // Create the router
    let app = SseTransport::create_router(server);

    // Bind to localhost:3000
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Server running on http://{}", addr);
    println!("Try calling the calculator tools with:");
    println!(r#"# First get a connection ID:
curl http://localhost:3000/sse

# Then initialize the connection:
curl -X POST http://localhost:3000/message?sessionId=$SESSION_ID \
    -H "Content-Type: application/json" \
    -d '{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"0.1.0","capabilities":{{"experimental":{{}},"sampling":{{}},"roots":{{"listChanged":false}}}},"clientInfo":{{"name":"curl","version":"1.0.0"}}}}}}'

# Then use it to make calls:
curl -X POST http://localhost:3000/message?sessionId=$SESSION_ID \
    -H "Content-Type: application/json" \
    -d '{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"calculator_add","arguments":{{"a":2,"b":3}}}}}}'

# Try other operations:
curl -X POST http://localhost:3000/message?sessionId=$SESSION_ID \
    -H "Content-Type: application/json" \
    -d '{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"calculator_multiply","arguments":{{"a":4,"b":5}}}}}}'
"#);

    // Start the server
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
} 