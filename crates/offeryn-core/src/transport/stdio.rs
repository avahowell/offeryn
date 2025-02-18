use crate::McpServer;
use jsonrpc_core::{Call, Error, Failure, Id, Output, Request, Response, Version};
use std::sync::Arc;
use tokio::{
    io::{
        stdin, stdout, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, BufWriter,
    },
    sync::mpsc,
};

pub struct StdioTransport<R, W>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    server: Arc<McpServer>,
    stdin: R,
    stdout: W,
}

impl StdioServerTransport<tokio::io::Stdin, tokio::io::Stdout> {
    pub fn new(server: Arc<McpServer>) -> Self {
        Self {
            server,
            stdin: stdin(),
            stdout: stdout(),
        }
    }
}

impl<R, W> StdioServerTransport<R, W>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    pub fn with_streams(server: Arc<McpServer>, stdin: R, stdout: W) -> Self {
        Self {
            server,
            stdin,
            stdout,
        }
    }

    async fn read_message<RR: AsyncRead + Unpin>(
        reader: &mut BufReader<RR>,
    ) -> Result<Vec<u8>, std::io::Error> {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "EOF",
            ));
        }
        Ok(line.into_bytes())
    }

    async fn write_message<WW: AsyncWrite + Unpin>(
        writer: &mut BufWriter<WW>,
        message: &[u8],
    ) -> Result<(), std::io::Error> {
        writer.write_all(message).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
        Ok(())
    }

    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let (tx, mut rx) = mpsc::channel(100);
        let mut reader = BufReader::new(self.stdin);

        let response_handler = tokio::spawn({
            let mut writer = BufWriter::new(self.stdout);
            async move {
                while let Some(response) = rx.recv().await {
                    let response_json = serde_json::to_vec(&response)?;
                    Self::write_message(&mut writer, &response_json).await?;
                }
                Ok::<_, std::io::Error>(())
            }
        });

        loop {
            let message = match Self::read_message(&mut reader).await {
                Ok(msg) => msg,
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(_) => continue,
            };

            let request: Request = match serde_json::from_slice(&message) {
                Ok(req) => req,
                Err(_) => {
                    let id = serde_json::from_slice::<serde_json::Value>(&message)
                        .ok()
                        .and_then(|v| v.get("id").cloned())
                        .and_then(|id| id.as_u64())
                        .map_or(Id::Num(0), Id::Num);

                    let error_response = Response::Single(Output::Failure(Failure {
                        jsonrpc: Some(Version::V2),
                        error: Error::parse_error(),
                        id,
                    }));
                    let _ = tx.send(error_response).await;
                    continue;
                }
            };

            match self.server.handle_request(request.clone()).await {
                Ok(response) => {
                    if tx.send(response).await.is_err() {
                        break;
                    }
                }
                Err(_) => {
                    let id = match &request {
                        Request::Single(Call::MethodCall(m)) => m.id.clone(),
                        Request::Single(Call::Notification(_)) => Id::Num(0),
                        _ => Id::Num(0),
                    };
                    let error_response = Response::Single(Output::Failure(Failure {
                        jsonrpc: Some(Version::V2),
                        error: Error::internal_error(),
                        id,
                    }));
                    if tx.send(error_response).await.is_err() {
                        break;
                    }
                }
            }
        }

        drop(tx);
        let _ = response_handler.await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use offeryn_derive::tool;
    use serde_json::json;
    use tokio::io::{duplex, DuplexStream};

    #[derive(Default)]
    struct Calculator {}

    #[tool]
    impl Calculator {
        async fn add(&self, a: i64, b: i64) -> Result<i64, String> {
            Ok(a + b)
        }
    }

    #[tokio::test]
    async fn test_calculator_add() {
        let server = Arc::new(McpServer::new("test-server", "1.0.0"));
        let calc = Calculator::default();
        server.register_tools(calc).await;

        let (client_reader, server_writer) = duplex(1024);
        let (server_reader, client_writer) = duplex(1024);

        let transport = StdioServerTransport::with_streams(server, server_reader, server_writer);
        let server_task = tokio::spawn(async move {
            transport.run().await.unwrap();
        });

        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "calculator_add",
                "arguments": {
                    "a": 2,
                    "b": 3
                }
            }
        });

        let mut client_writer = BufWriter::new(client_writer);
        let request_json = serde_json::to_vec(&request).unwrap();
        StdioServerTransport::<DuplexStream, DuplexStream>::write_message(
            &mut client_writer,
            &request_json,
        )
        .await
        .unwrap();

        let mut client_reader = BufReader::new(client_reader);
        let response_bytes =
            StdioServerTransport::<DuplexStream, DuplexStream>::read_message(&mut client_reader)
                .await
                .unwrap();
        let response: Response = serde_json::from_slice(&response_bytes).unwrap();

        server_task.abort();

        match response {
            Response::Single(Output::Success(success)) => {
                let result = success.result;
                let content = result.get("content").unwrap().as_array().unwrap();
                let text = content[0].get("text").unwrap().as_str().unwrap();
                assert_eq!(text, "5"); // 2 + 3 = 5
            }
            _ => panic!("Expected successful response"),
        }
    }
}
