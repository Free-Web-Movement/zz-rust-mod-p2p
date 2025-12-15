use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{ AsyncReadExt, AsyncWriteExt };
use tokio::net::{ TcpListener, TcpStream };
use tokio_util::sync::CancellationToken;

use crate::context::{ self, Context };
use crate::defines::Listener;
use crate::http::HTTPHandler;

/// 默认 TCP 读取缓冲区
pub const TCP_BUFFER_LENGTH: usize = 8 * 1024;

/// HTTP 探测用 peek 缓冲区
pub const PEEK_TCP_BUFFER_LENGTH: usize = 1024;

#[derive(Clone)]
pub struct TCPHandler {
    ip: String,
    port: u16,
    context: Arc<Context>,
    listener: Arc<TcpListener>,
}

impl TCPHandler {
    /// 创建并 bind TCPHandler
    pub async fn bind(ip: &str, port: u16, context: Arc<Context>) -> anyhow::Result<Arc<Self>> {
        let addr = format!("{}:{}", ip, port);
        let listener = TcpListener::bind(&addr).await?;

        println!("TCP listening on {}", addr);

        Ok(
            Arc::new(Self {
                ip: ip.to_string(),
                port,
                context,
                listener: Arc::new(listener),
            })
        )
    }

    /// 启动 accept loop（阻塞）
    pub async fn start(self: Arc<Self>, token: CancellationToken) -> anyhow::Result<()> {
        let cloned = token.clone();
        loop {
            tokio::select! {
                _ = cloned.cancelled() => {
                    println!("TCP listener shutting down");
                    break;
                }
                res = self.listener.accept() => {
                    match res {
                        Ok((socket, addr)) => {
                            let this = self.clone();
                            let cloned = token.clone();
                            tokio::spawn(async move {
                                this.handle_connection(socket, addr, cloned).await;
                            });
                        }
                        Err(e) => eprintln!("TCP accept error: {:?}", e),
                    }
                }
            }
        }
        Ok(())
    }

    /// 每个 TCP 连接的唯一入口
    async fn handle_connection(
        self: Arc<Self>,
        mut socket: TcpStream,
        addr: std::net::SocketAddr,
        token: CancellationToken
    ) {
        println!("TCP connection from {}", addr);

        // 👇 只判断一次 HTTP
        match HTTPHandler::is_http_connection(&socket).await {
            Ok(true) => {
                println!("HTTP connection detected from {}", addr);
                let _ = HTTPHandler::new(
                    &addr.ip().to_string(),
                    addr.port(),
                    socket,
                    self.context.clone()
                ).start(token).await;
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
            tokio::select! {
                _ = token.cancelled() => {
                    println!("TCP connection shutdown {}", addr);
                    break;
                }

                res = socket.read(&mut buf) => {
                    match res {
                        Ok(0) => break,
                        Ok(n) => {
                            if self.on_data(&buf[..n], &mut socket, &addr).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }
        let _ = socket.shutdown().await;
        println!("TCP connection closed {}", addr);
    }

    async fn on_data(
        &self,
        data: &[u8],
        socket: &mut TcpStream,
        addr: &std::net::SocketAddr
    ) -> anyhow::Result<()> {
        println!("TCP received {} bytes from {}", data.len(), addr);

        // 默认行为：echo（保持你现在的测试通过）
        socket.write_all(data).await?;

        Ok(())
    }
}

#[async_trait]
impl Listener for TCPHandler {
    async fn run(&mut self, token: CancellationToken) -> anyhow::Result<()> {
        // start expects an Arc<Self>, so clone the handler into an Arc and call start on it
        let arc_self = Arc::new(self.clone());
        arc_self.start(token).await
    }
    async fn new(ip: &String, port: u16, context: Arc<Context>) -> Arc<Self> {
        TCPHandler::bind(ip, port, context).await.unwrap()
    }
    async fn stop(self: Arc<Self>, token: CancellationToken) -> anyhow::Result<()> {
        // TCPListener does not have a built-in stop method.
        // You would need to implement your own mechanism to stop the listener.
        token.cancel();
        let _ = TcpStream::connect(format!("{}:{}", self.ip, self.port)).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{ AsyncReadExt, AsyncWriteExt };
    use tokio::net::TcpStream;
    use zz_account::address::FreeWebMovementAddress;

    #[tokio::test]
    async fn test_tcp_echo() -> anyhow::Result<()> {
        let ip = "127.0.0.1";
        let port = 18000;
        let address = FreeWebMovementAddress::random();
        let context = Arc::new(Context::new(address));
        let server = TCPHandler::bind(ip, port, context).await?;
        let token = CancellationToken::new();

        tokio::spawn(server.clone().start(token.clone()));

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let mut stream = TcpStream::connect(format!("{}:{}", ip, port)).await?;
        let msg = b"hello tcp";

        stream.write_all(msg).await?;

        let mut buf = vec![0u8; msg.len()];
        stream.read_exact(&mut buf).await?;

        assert_eq!(buf, msg);

        server.stop(token).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_tcp_multiple_clients() -> anyhow::Result<()> {
        let ip = "127.0.0.1";
        let port = 18001;

        let address = FreeWebMovementAddress::random();
        let context = Arc::new(Context::new(address));
        let server = TCPHandler::bind(ip, port, context).await?;
        let token = CancellationToken::new();

        let server_task = tokio::spawn(server.clone().start(token.clone()));

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

        server.stop(token).await?;
        let _ = server_task.await?; // 等待 listener task 退出

        Ok(())
    }
}
