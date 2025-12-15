use std::sync::Arc;
use async_trait::async_trait;
use tokio::{ net::UdpSocket, sync::Mutex };
use crate::defines::Listener;

pub const PEEK_UDP_BUFFER_LENGTH: usize = 1024;

#[derive(Debug, Clone)]
pub struct UDPHandler {
    ip: String,
    port: u16,
    listener: Option<Arc<Mutex<UdpSocket>>>,
}

impl UDPHandler {
    pub async fn bind(ip: &str, port: u16) -> anyhow::Result<Arc<Self>> {
        Ok(
            Arc::new(UDPHandler {
                ip: ip.to_string(),
                port,
                listener: None,
            })
        )
    }

    pub async fn start(self: Arc<Self>) -> anyhow::Result<()> {
        let udp_addr = format!("{}:{}", self.ip, self.port);
        let socket = UdpSocket::bind(&udp_addr).await?;
        println!("UDP listening on {}", udp_addr);

        let socket = Arc::new(Mutex::new(socket));
        let socket_clone = Arc::clone(&socket);

        tokio::spawn(async move {
            let mut buf = vec![0u8; PEEK_UDP_BUFFER_LENGTH];
            loop {
                let mut sock = socket_clone.lock().await;
                match sock.recv_from(&mut buf).await {
                    Ok((n, src)) => {
                        println!("UDP received {} bytes from {}: {:?}", n, src, &buf[..n]);
                        let _ = sock.send_to(&buf[..n], &src).await;
                    }
                    Err(e) => eprintln!("UDP recv error: {:?}", e),
                }
            }
        });

        Ok(())
    }
}

#[async_trait]
impl Listener for UDPHandler {
    async fn run(&mut self) -> anyhow::Result<()> {
        let arc_self = Arc::new(self.clone());
        arc_self.start().await
    }
    async fn new(ip: &String, port: u16) -> Arc<Self> {
        UDPHandler::bind(ip, port).await.unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::ser;
    use tokio::net::UdpSocket;

    #[tokio::test]
    async fn test_udp_echo() -> anyhow::Result<()> {
        let ip = "127.0.0.1";
        let port = 19000;

        let server = UDPHandler::bind(ip, port).await?;
        let server_clone = Arc::clone(&server);
        server_clone.start().await?; // 启动后台服务器

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let client = UdpSocket::bind("0.0.0.0:0").await?;
        let server_addr = format!("{}:{}", ip, port);

        let msg = b"hello udp";
        client.send_to(msg, &server_addr).await?;

        let mut buf = vec![0u8; PEEK_UDP_BUFFER_LENGTH];
        let (n, _) = client.recv_from(&mut buf).await?;
        assert_eq!(&buf[..n], msg);

        Ok(())
    }

    #[tokio::test]
    async fn test_udp_multiple_messages() -> anyhow::Result<()> {
        let ip = "127.0.0.1";
        let port = 19001;

        let server = UDPHandler::bind(ip, port).await?;
        server.start().await?;

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let client = UdpSocket::bind("0.0.0.0:0").await?;
        let server_addr = format!("{}:{}", ip, port);

        let messages = vec![b"msg1", b"msg2", b"msg3"];
        for msg in messages.iter() {
            client.send_to(*msg, &server_addr).await?;
            let mut buf = vec![0u8; PEEK_UDP_BUFFER_LENGTH];
            let (n, _) = client.recv_from(&mut buf).await?;
            assert_eq!(&buf[..n], *msg);
        }

        Ok(())
    }
}
