# mcp-rs

mcp-rs is a Rust implementation of [modelcontextprotocol](https://modelcontextprotocol.io/), a standard protocol for empowering large language models with tools.

- [x] JSON-RPC core MCP server protocol
- [x] Procedural macro for tool generation
- [x] Server-Sent Events (SSE) transport
- [ ] Stdio transport 
- [ ] Client protocol
- [ ] WebSocket transport 
- [ ] Streaming responses


## Example

```rust
use mcp_derive::mcp_tool;
use mcp_rs::{McpServer, SseTransport};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
/// A simple calculator that can perform basic arithmetic operations
#[derive(Default, Clone)]
struct Calculator {}

// The mcp_tool proc macro generates tool methods that are enumerated to MCP clients.
// Docstrings are used as tool descriptions. This example will generate:
// {
//     "name": "calculator_divide",
//     "description": "Divide two numbers",
//     "inputSchema": {
//         "type": "object",
//         "properties": {
//             "a": {
//                 "description": "Dividend - the number to be divided",
//                 "format": "int64",
//                 "type": "integer"
//             },
//             "b": {
//                 "description": "Divisor - the number to divide by",
//                 "format": "int64",
//                 "type": "integer"
//             }
//         },
//         "required": [
//             "a",
//             "b"
//         ]
//     }
// }
// ... (cont)
//
// Results are handled as you would expect, with the proper JSON-RPC error code.
#[mcp_tool]
impl Calculator {
    /// Add two numbers
    /// # Parameters
    /// * `a` - First value to add
    /// * `b` - Second value to add
    async fn add(&self, a: i64, b: i64) -> i64 {
        a + b
    }

    /// Subtract two numbers
    /// # Parameters
    /// * `a` - Number to subtract from
    /// * `b` - Number to subtract
    async fn subtract(&self, a: i64, b: i64) -> i64 {
        a - b
    }

    /// Multiply two numbers
    /// # Parameters
    /// * `a` - First factor to multiply
    /// * `b` - Second factor to multiply
    async fn multiply(&self, a: i64, b: i64) -> i64 {
        a * b
    }

    /// Divide two numbers
    /// # Parameters
    /// * `a` - Dividend - the number to be divided
    /// * `b` - Divisor - the number to divide by
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
    let server = Arc::new(McpServer::new("calculator", "1.0.0"));

    server.register_tools(Calculator::default()).await;

    let app = SseTransport::create_router(server);
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Server running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    // You can now connect to the server using a MCP client in SSE mode.
}
```
