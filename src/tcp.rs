use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::http::HTTPHandler;

/// 默认 TCP 读取缓冲区
pub const TCP_BUFFER_LENGTH: usize = 8 * 1024;

/// HTTP 探测用 peek 缓冲区
pub const PEEK_TCP_BUFFER_LENGTH: usize = 1024;

pub struct TCPHandler {
    ip: String,
    port: u16,
    listener: Arc<TcpListener>,
}

impl TCPHandler {
    /// 创建并 bind TCPHandler
    pub async fn bind(ip: &str, port: u16) -> anyhow::Result<Arc<Self>> {
        let addr = format!("{}:{}", ip, port);
        let listener = TcpListener::bind(&addr).await?;

        println!("TCP listening on {}", addr);

        Ok(Arc::new(Self {
            ip: ip.to_string(),
            port,
            listener: Arc::new(listener),
        }))
    }

    /// 启动 accept loop（阻塞）
    pub async fn start(self: Arc<Self>) -> anyhow::Result<()> {
        loop {
            let (socket, addr) = self.listener.accept().await?;
            let this = self.clone();

            tokio::spawn(async move {
                this.handle_connection(socket, addr).await;
            });
        }
    }

    /// 每个 TCP 连接的唯一入口
    async fn handle_connection(self: Arc<Self>, mut socket: TcpStream, addr: std::net::SocketAddr) {
        println!("TCP connection from {}", addr);

        // 👇 只判断一次 HTTP
        match HTTPHandler::is_http_connection(&socket).await {
            Ok(true) => {
                println!("HTTP connection detected from {}", addr);
                HTTPHandler::new(&addr.ip().to_string(), addr.port(), socket)
                    .start()
                    .await;
                return;
            }
            Ok(false) => {}
            Err(e) => {
                eprintln!("HTTP detection error: {:?}", e);
                return;
            }
        }

        // 👇 普通 TCP 处理
        let mut buf = vec![0u8; TCP_BUFFER_LENGTH];

        loop {
            match socket.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    // 默认 echo（测试 & 占位）
                    if self
                        .on_tcp_data(&buf[..n], &mut socket, &addr)
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("TCP read error {}: {:?}", addr, e);
                    break;
                }
            }
        }

        println!("TCP connection closed {}", addr);
    }

    async fn on_tcp_data(
        &self,
        data: &[u8],
        socket: &mut TcpStream,
        addr: &std::net::SocketAddr,
    ) -> anyhow::Result<()> {
        println!("TCP received {} bytes from {}", data.len(), addr);

        // 默认行为：echo（保持你现在的测试通过）
        socket.write_all(data).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    #[tokio::test]
    async fn test_tcp_echo() -> anyhow::Result<()> {
        let ip = "127.0.0.1";
        let port = 18000;

        let server = TCPHandler::bind(ip, port).await?;
        tokio::spawn(server.clone().start());

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let mut stream = TcpStream::connect(format!("{}:{}", ip, port)).await?;
        let msg = b"hello tcp";

        stream.write_all(msg).await?;

        let mut buf = vec![0u8; msg.len()];
        stream.read_exact(&mut buf).await?;

        assert_eq!(buf, msg);

        Ok(())
    }

    #[tokio::test]
    async fn test_tcp_multiple_clients() -> anyhow::Result<()> {
        let ip = "127.0.0.1";
        let port = 18001;

        let server = TCPHandler::bind(ip, port).await?;
        tokio::spawn(server.clone().start());

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let mut c1 = TcpStream::connect(format!("{}:{}", ip, port)).await?;
        let mut c2 = TcpStream::connect(format!("{}:{}", ip, port)).await?;

        let msg1 = b"client1";
        let msg2 = b"client2";

        c1.write_all(msg1).await?;
        c2.write_all(msg2).await?;

        let mut buf1 = vec![0u8; msg1.len()];
        let mut buf2 = vec![0u8; msg2.len()];

        c1.read_exact(&mut buf1).await?;
        c2.read_exact(&mut buf2).await?;

        assert_eq!(buf1, msg1);
        assert_eq!(buf2, msg2);

        Ok(())
    }
}
