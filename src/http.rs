use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::context::Context;
use crate::ws::WebSocketHandler;

pub const HTTP_BUFFER_LENGTH: usize = 8 * 1024;

pub struct HTTPHandler {
    stream: Arc<Mutex<TcpStream>>,
    context: Arc<Context>,
}

impl HTTPHandler {
    pub async fn is_http_connection(stream: &TcpStream) -> anyhow::Result<bool> {
        let mut buf = [0u8; 8];
        let n = stream.peek(&mut buf).await?;
        if n == 0 {
            return Ok(false);
        }
        let s = std::str::from_utf8(&buf[..n]).unwrap_or("");
        Ok(matches!(s, "GET" | "POST" | "PUT" | "DELETE" | "HEAD" | "OPTIONS" | "PATCH"))
    }

    pub fn new(stream: Arc<Mutex<TcpStream>>, context: Arc<Context>) -> Self {
        Self {
            stream,
            context,
        }
    }

    /// ⚠️ 注意：这里 **不 spawn**
    pub async fn start(&self, token: CancellationToken) -> anyhow::Result<()> {
        let mut buf = vec![0u8; HTTP_BUFFER_LENGTH];

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    break;
                }

                res = async {
                    let mut stream = self.stream.lock().await;
                    stream.read(&mut buf).await
                } => {
                    let n = res?;
                    if n == 0 {
                        break;
                    }

                    let data = &buf[..n];

                    // WebSocket upgrade
                    if WebSocketHandler::is_websocket_request(data) {
                        let ws = Arc::new(WebSocketHandler::new(
                            self.stream.clone(),
                            self.context.clone(),
                        ));
                        ws.respond_websocket_handshake(data).await?;
                        break;
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::net::{ TcpListener, TcpStream };
    use tokio::io::{ AsyncReadExt, AsyncWriteExt };
    use tokio::sync::Mutex;
    use tokio_util::sync::CancellationToken;
    use zz_account::address::FreeWebMovementAddress;
    use crate::context::Context;

    use super::HTTPHandler;
    static WS_KEY: &str = "dGhlIHNhbXBsZSBub25jZQ==";
    static WS_REQ: &str =
        "\
GET /chat HTTP/1.1\r\n\
Host: localhost\r\n\
Upgrade: websocket\r\n\
Connection: Upgrade\r\n\
Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
Sec-WebSocket-Version: 13\r\n\
\r\n";

    #[tokio::test]
    async fn test_websocket_handshake() -> anyhow::Result<()> {
        let ip = "127.0.0.1".to_string();
        let port = 23000;
        let addr = format!("{}:{}", ip, port);

        let token = CancellationToken::new();

        // Start a raw TCP listener
        let listener = TcpListener::bind(&addr).await?;

        // Spawn server task
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let address = FreeWebMovementAddress::random();
            let context = Arc::new(Context::new(ip.clone(), port, address));
            let handler = HTTPHandler::new(Arc::new(Mutex::new(stream)), context);
            let _ = handler.start(token).await;
        });

        // Wait for server to be ready
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        // Connect as client
        let mut client = TcpStream::connect(&addr).await?;

        // Send WebSocket handshake
        client.write_all(WS_REQ.as_bytes()).await?;

        // Read server response
        let mut buf = vec![0u8; 1024];
        let n = client.read(&mut buf).await?;
        let resp = String::from_utf8_lossy(&buf[..n]);
        println!("Handshake response:\n{}", resp);

        // --- Validate status line ---
        assert!(resp.contains("101 Switching Protocols"));

        // --- Validate headers ---
        assert!(resp.to_ascii_lowercase().contains("upgrade: websocket"));
        assert!(resp.to_ascii_lowercase().contains("connection: upgrade"));

        // --- Validate Sec-WebSocket-Accept ---
        use sha1::{ Sha1, Digest };
        use base64::engine::general_purpose::STANDARD as BASE64;
        use base64::Engine;

        let mut hasher = Sha1::new();
        hasher.update(format!("{}258EAFA5-E914-47DA-95CA-C5AB0DC85B11", WS_KEY).as_bytes());
        let expected_accept = BASE64.encode(hasher.finalize());

        assert!(resp.contains(&expected_accept), "Sec-WebSocket-Accept mismatch");

        Ok(())
    }
}
