use tokio::net::TcpListener;
use tokio::io::{ AsyncReadExt, AsyncWriteExt };

use crate::http::HTTPHandler;
use crate::share::PEEK_STREAM_BUFFER_LENGTH;

pub struct TCPHandler {
    ip: String,
    port: u16,
    listener: Option<TcpListener>,
}

impl TCPHandler {
    pub fn new(ip: &String, port: u16) -> Self {
        TCPHandler { ip: ip.clone(), port, listener: None }
    }

    pub async fn start(&mut self) -> anyhow::Result<()> {
        // 启动 TCP 监听
        let tcp_addr = format!("{}:{}", self.ip, self.port);
        let listener = TcpListener::bind(&tcp_addr).await?;
        self.listener = Some(listener);
        println!("TCP listening on {}", tcp_addr);
        println!("Starting TCP server on {}:{}", self.ip, self.port);
        self.handling().await;
        Ok(())
    }

    async fn handling(&mut self) {
        // 将 listener 从 self 中取出并移入后台任务
        if let Some(listener) = self.listener.take() {
            tokio::spawn(async move {
                let listener = listener;
                loop {
                    match listener.accept().await {
                        Ok((mut socket, addr)) => {
                            println!("TCP connection from {}", addr);
                            tokio::spawn(async move {
                                let mut buf = vec![0u8; PEEK_STREAM_BUFFER_LENGTH];
                                loop {
                                    if
                                        HTTPHandler::is_http_connection(&socket).await.unwrap_or(
                                            false
                                        )
                                    {
                                        println!("HTTP connection detected from {}", addr);
                                        // 将 HTTP 处理交给一个新的任务来独占 socket，并结束当前连接处理循环
                                        tokio::spawn(async move {
                                            let http_handler = HTTPHandler::new(
                                                &addr.ip().to_string(),
                                                addr.port(),
                                                socket
                                            );
                                            http_handler.start().await;
                                        });
                                        break;
                                    }
                                    match socket.read(&mut buf).await {
                                        Ok(0) => {
                                            break;
                                        } // 连接关闭
                                        Ok(n) => {
                                            println!(
                                                "TCP received {} bytes from {}: {:?}",
                                                n,
                                                addr,
                                                &buf[..n]
                                            );
                                            let _ = socket.write_all(&buf[..n]).await;
                                        }
                                        Err(e) => {
                                            eprintln!("TCP read error: {:?}", e);
                                            break;
                                        }
                                    }
                                }
                                println!("TCP connection closed {}", addr);
                            });
                        }
                        Err(e) => eprintln!("Accept error: {:?}", e),
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpStream;
    use tokio::io::{ AsyncReadExt, AsyncWriteExt };

    #[tokio::test]
    async fn test_tcp_echo() -> anyhow::Result<()> {
        let ip = "127.0.0.1".to_string();
        let port = 18000; // 测试专用端口，避免冲突
        let mut server = TCPHandler::new(&ip, port);

        // 启动 TCPHandler，在后台 task
        tokio::spawn(async move {
            server.start().await.unwrap();
        });

        // 等待服务器启动
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // 连接到服务器
        let mut stream = TcpStream::connect(format!("{}:{}", ip, port)).await?;
        let msg = b"hello tcp";

        // 写入消息
        stream.write_all(msg).await?;

        // 读回显
        let mut buf = vec![0u8; msg.len()];
        stream.read_exact(&mut buf).await?;

        assert_eq!(buf, msg, "TCP echo mismatch");

        stream.shutdown().await?;

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        Ok(())
    }

    #[tokio::test]
    async fn test_tcp_multiple_clients() -> anyhow::Result<()> {
        let ip = "127.0.0.1".to_string();
        let port = 18001; // 测试专用端口
        let mut server = TCPHandler::new(&ip, port);

        tokio::spawn(async move {
            server.start().await.unwrap();
        });

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // 同时启动两个客户端
        let mut stream1 = TcpStream::connect(format!("{}:{}", ip, port)).await?;
        let mut stream2 = TcpStream::connect(format!("{}:{}", ip, port)).await?;

        let msg1 = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let msg2 = b"client2";

        // 写入消息
        stream1.write_all(msg1).await?;
        stream2.write_all(msg2).await?;

        let mut buf1 = vec![0u8; msg1.len()];
        let mut buf2 = vec![0u8; msg2.len()];

        stream1.read_exact(&mut buf1).await?;
        stream2.read_exact(&mut buf2).await?;

        assert_eq!(buf1, msg1);
        assert_eq!(buf2, msg2);

        stream1.shutdown().await?;
        stream2.shutdown().await?;
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        Ok(())
    }
}
