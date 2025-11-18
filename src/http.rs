use base64::Engine;
use tokio::net::TcpStream;
use tokio::io::{ AsyncReadExt, AsyncWriteExt };
use crate::share::HTTP_BUFFER_LENGTH;
use crate::share::{
    HEADER_SEC_WEBSOCKET_KEY,
    HEADER_UPGRADE, WEBSOCKET_MAGIC_GUID, WEBSOCKET_UPGRADE_VALUE, CONNECTION_UPGRADE_VALUE,
};

pub struct HTTPHandler {
    ip: String,
    port: u16,
    stream: TcpStream,
}

impl HTTPHandler {
    pub fn new(ip: &String, port: u16, stream: TcpStream) -> Self {
        HTTPHandler { ip: ip.clone(), port, stream }
    }

    pub async fn start(self) {
        tokio::spawn(async move {
            let mut stream = self.stream;

            loop {
                let mut buf = vec![0u8; HTTP_BUFFER_LENGTH];

                match stream.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        println!("HTTP received {} bytes: {:?}", n, &buf[..n]);

                        // 一旦 header 包含 WebSocket Upgrade → 切换到 WS handler
                        if Self::is_websocket_request(&buf[..n]) {
                            println!("Detected WebSocket upgrade request");
                            let _ = Self::respond_websocket_handshake(&mut stream, &buf[..n]).await;
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

    /// 判断是否为 WebSocket Upgrade
    fn is_websocket_request(data: &[u8]) -> bool {
        let s = String::from_utf8_lossy(data);
        s.contains(HEADER_UPGRADE) && s.to_lowercase().contains(WEBSOCKET_UPGRADE_VALUE)
    }

    /// 返回 WebSocket 握手响应
    async fn respond_websocket_handshake(
        stream: &mut TcpStream,
        req: &[u8],
    ) -> anyhow::Result<()> {
        use sha1::{Digest, Sha1};
        use base64::engine::general_purpose::STANDARD as BASE64;

        let req_str = String::from_utf8_lossy(req);

        // 提取 Sec-WebSocket-Key
        let key_line = req_str
            .lines()
            .find(|line| line.starts_with(HEADER_SEC_WEBSOCKET_KEY))
            .ok_or_else(|| anyhow::anyhow!("Missing Sec-WebSocket-Key"))?;

        let key = key_line.split(':').nth(1).unwrap().trim();

        // SHA1(key + magic)
        let mut hasher = Sha1::new();
        hasher.update(key.as_bytes());
        hasher.update(WEBSOCKET_MAGIC_GUID.as_bytes());

        let accept_key = BASE64.encode(hasher.finalize());

        let response = format!(
            "HTTP/1.1 101 Switching Protocols\r\n\
             Connection: {CONNECTION_UPGRADE_VALUE}\r\n\
             Upgrade: {WEBSOCKET_UPGRADE_VALUE}\r\n\
             Sec-WebSocket-Accept: {accept_key}\r\n\
             \r\n"
        );

        stream.write_all(response.as_bytes()).await?;
        Ok(())
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
