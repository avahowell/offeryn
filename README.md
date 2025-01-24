# offeryn

offeryn (Welsh for tool; implement), is a Rust implementation of [modelcontextprotocol](https://modelcontextprotocol.io/), a standard protocol for empowering large language models with tools.

It aims to offer an easy to use API that allows developers to quickly create and expose tools written in Rust to AI agents.

- [x] JSON-RPC core MCP server protocol
- [x] Procedural macro for tool generation
- [x] Server-Sent Events (SSE) transport
- [x] Stdio transport
- [ ] Client protocol
- [ ] WebSocket transport
- [ ] Streaming responses

## Example (Stdio)
```rust
use offeryn::prelude::*;
use offeryn::{StdioTransport, McpServer};
use std::sync::Arc;
use async_trait;

/// A simple calculator that can perform basic arithmetic operations
#[derive(Default, Clone)]
struct Calculator {}

// The mcp_tool proc macro generates tool methods that are enumerated to MCP clients.
// It can handle arbitrary types, including Result<T> and Option<T>.
// A correct json-schema is generated for each function signature of the tool, based on the Rust type.
// Docstrings are used as tool descriptions, passed along in MCP to inform the AI agent of the tool's purpose.
//
// This example will generate:
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
// ... (cont... calculator_add, calculator_subtract, calculator_multiply)
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
```

Servers configured with stdio transport as above can be hooked to Claude Desktop as normal (see https://modelcontextprotocol.io/quickstart/user, build a binary with mcp-rs and point at it in mcpServers).


## Example (SSE)

```rust
use offeryn::prelude::*;
use offeryn::{SseTransport, McpServer};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
/// A simple calculator that can perform basic arithmetic operations
#[derive(Default, Clone)]
struct Calculator {}

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
