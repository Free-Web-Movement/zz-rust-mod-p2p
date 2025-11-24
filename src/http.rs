use tokio::net::TcpStream;
use tokio::io::{ AsyncReadExt, AsyncWriteExt };
use crate::ws::WebSocketHandler;

pub const PEEK_HTTP_BUFFER_LENGTH: usize = 4096;

/// ==============================
///        HTTP 常量
/// ==============================

/// 默认 HTTP 读取缓冲区大小
pub const HTTP_BUFFER_LENGTH: usize = 8 * 1024; // 8 KB


pub struct HTTPHandler {
    ip: String,
    port: u16,
    stream: TcpStream,
}

impl HTTPHandler {
    pub async fn is_http_connection(stream: &TcpStream) -> anyhow::Result<bool> {
        let mut buf = [0u8; 8]; // 前 8 个字节足够识别方法
        let n = stream.peek(&mut buf).await?; // peek 不消费数据
        if n == 0 {
            return Ok(false);
        }
        let s = std::str::from_utf8(&buf[..n]).unwrap_or("");
        let http_methods = [
            "GET",
            "POST",
            "PUT",
            "DELETE",
            "HEAD",
            "OPTIONS",
            "PATCH",
            "CONNECT",
            "TRACE",
        ];
        for method in &http_methods {
            if s.starts_with(method) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn new(ip: &String, port: u16, stream: TcpStream) -> Self {
        HTTPHandler { ip: ip.clone(), port, stream }
    }

    pub async fn start(self) {
        tokio::spawn(async move {
            let mut stream = self.stream;

            loop {
                let mut buf = vec![0u8; HTTP_BUFFER_LENGTH];

                match stream.read(&mut buf).await {
                    Ok(0) => {
                        break;
                    }
                    Ok(n) => {
                        println!("HTTP received {} bytes: {:?}", n, &buf[..n]);

                        // 一旦 header 包含 WebSocket Upgrade → 切换到 WS handler
                        if WebSocketHandler::is_websocket_request(&buf[..n]) {
                            println!("Detected WebSocket upgrade request");
                            let _ = WebSocketHandler::respond_websocket_handshake(
                                &mut stream,
                                &buf[..n]
                            ).await;
                            break;
                        }

                        let _ = stream.write_all(&buf[..n]).await;
                    }
                    Err(e) => {
                        eprintln!("HTTP read error: {:?}", e);
                        break;
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use tokio::net::{ TcpListener, TcpStream };
    use tokio::io::{ AsyncReadExt, AsyncWriteExt };
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

        // Start a raw TCP listener
        let listener = TcpListener::bind(&addr).await?;

        // Spawn server task
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();

            let handler = HTTPHandler::new(&ip, port, stream);
            handler.start().await;
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
