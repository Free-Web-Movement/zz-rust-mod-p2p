use std::sync::Arc;

use tokio::{net::UdpSocket, sync::Mutex};

pub const PEEK_UDP_BUFFER_LENGTH: usize = 1024;

pub struct UDPHandler {
    ip: String,
    port: u16,
    listener: Option<Arc<Mutex<UdpSocket>>>
}

impl UDPHandler {

    pub fn new(ip: &String, port: u16) -> Self {
        UDPHandler { ip: ip.clone(), port, listener: None }
    }

    pub async fn start(&mut self) -> anyhow::Result<()> {
        // 启动 UDP 监听
        let udp_addr = format!("{}:{}", self.ip, self.port);
        self.listener = Some(Arc::new(Mutex::new(UdpSocket::bind(&udp_addr).await?)));
        println!("UDP listening on {}", udp_addr);
        self.handling().await;
        Ok(())
    }

    async fn handling(&mut self) {
        // 将 listener 从 self 中取出并移入后台任务
        if let Some(listener) = self.listener.take() {
            let cloned = Arc::clone(&listener);
            tokio::spawn(async move {
                let listener = cloned.lock().await;
                let mut buf = vec![0u8; PEEK_UDP_BUFFER_LENGTH];
                loop {
                    match listener.recv_from(&mut buf).await {
                        Ok((n, src)) => {
                            println!("UDP received {} bytes from {}: {:?}", n, src, &buf[..n]);
                            let _ = listener.send_to(&buf[..n], &src).await;
                        }
                        Err(e) => eprintln!("UDP recv error: {:?}", e),
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::UdpSocket;
    #[tokio::test]
    async fn test_udp_echo() -> anyhow::Result<()> {
        let ip = "127.0.0.1".to_string();
        let port = 19000; // 测试专用端口，避免冲突
        let mut server = UDPHandler::new(&ip, port);

        // 启动 UDPHandler，在后台 task
        tokio::spawn(async move {
            server.start().await.unwrap();
        });

        // 给服务器一点时间启动
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // 客户端 socket
        let client_socket = UdpSocket::bind("0.0.0.0:0").await?;
        let server_addr = format!("{}:{}", ip, port);

        let msg = b"hello udp";

        // 发送消息到服务器
        client_socket.send_to(msg, &server_addr).await?;

        // 接收回显
        let mut buf = vec![0u8; 1024];
        let (n, _) = client_socket.recv_from(&mut buf).await?;

        assert_eq!(&buf[..n], msg, "UDP echo mismatch");

        Ok(())
    }

    #[tokio::test]
    async fn test_udp_multiple_messages() -> anyhow::Result<()> {
        let ip = "127.0.0.1".to_string();
        let port = 19001;
        let mut server = UDPHandler::new(&ip, port);

        tokio::spawn(async move {
            server.start().await.unwrap();
        });

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let client_socket = UdpSocket::bind("0.0.0.0:0").await?;
        let server_addr = format!("{}:{}", ip, port);

        let messages = vec![b"msg1", b"msg2", b"msg3"];

        for msg in &messages {
            client_socket.send_to(*msg, &server_addr).await?;
            let mut buf = vec![0u8; PEEK_UDP_BUFFER_LENGTH];
            let (n, _) = client_socket.recv_from(&mut buf).await?;
            assert_eq!(&buf[..n], *msg, "UDP echo mismatch for message {:?}", msg);
        }

        Ok(())
    }
}
