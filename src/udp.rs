use crate::{context::Context, defines::Listener};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::{ net::UdpSocket, sync::Mutex };
use tokio_util::sync::CancellationToken;

pub const PEEK_UDP_BUFFER_LENGTH: usize = 1024;

#[derive(Clone)]
pub struct UDPHandler {
    ip: String,
    port: u16,
    context: Arc<Context>,
    listener: Arc<UdpSocket>,
}

impl UDPHandler {
    pub async fn bind(ip: &str, port: u16, context: Arc<Context>) -> anyhow::Result<Arc<Self>> {
        let udp_addr = format!("{}:{}", ip, port);
        let socket = Arc::new(UdpSocket::bind(&udp_addr).await?);
        println!("UDP listening on {}", udp_addr);

        Ok(
            Arc::new(UDPHandler {
                ip: ip.to_string(),
                port,
                listener: socket,
                context
            })
        )
    }

    pub async fn start(self: &Arc<Self>, token: CancellationToken) -> anyhow::Result<()> {
        let socket = Arc::new(Mutex::new(self.listener.clone()));
        let socket_clone = Arc::clone(&socket);

        let shutdown = token.clone();

        let cloned = self.clone();

        tokio::spawn(async move {
            let mut buf = vec![0u8; PEEK_UDP_BUFFER_LENGTH];
            let socket = socket_clone.lock().await;
            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => {
                        break;
                    }
                    res = socket.recv_from(&mut buf) => {
                        if let Ok((n, src)) = res {
                            // let _ = socket.send_to(&buf[..n], src).await;
                            let _ = cloned.clone().on_data(&buf[..n], &*socket, &src).await;
                        }
                    }
                }
            }
        });

        Ok(())
    }

    async fn on_data(
        self: Arc<Self>,
        data: &[u8],
        socket: &UdpSocket,
        des: &std::net::SocketAddr
    ) -> anyhow::Result<()> {
        println!("UDP received {} bytes from {}", data.len(), des);
        let _ = socket.send_to(data, des).await;
        Ok(())
    }
}

#[async_trait]
impl Listener for UDPHandler {
    async fn run(&mut self, token: CancellationToken) -> anyhow::Result<()> {
        let arc_self = Arc::new(self.clone());
        arc_self.start(token).await
    }
    async fn new(ip: &String, port: u16,context: Arc<Context>) -> Arc<Self> {
        UDPHandler::bind(ip, port, context).await.unwrap()
    }
    async fn stop(self: Arc<Self>, token: CancellationToken) -> anyhow::Result<()> {
        // UdpSocket does not have a built-in stop method.
        // You would need to implement your own mechanism to stop the listener.
        token.cancel();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::UdpSocket;
    use zz_account::address::FreeWebMovementAddress;

    #[tokio::test]
    async fn test_udp_echo() -> anyhow::Result<()> {
        let ip = "127.0.0.1";
        let port = 19000;

        let address = FreeWebMovementAddress::random();
        let context = Arc::new(Context::new(ip.to_string(), port, address));

        let server = UDPHandler::bind(ip, port, context).await?;
        let server_clone = Arc::clone(&server);
        let token = CancellationToken::new();
        server_clone.start(token).await?; // 启动后台服务器

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

        let address = FreeWebMovementAddress::random();
        let context = Arc::new(Context::new(ip.to_string(), port, address));

        let server = UDPHandler::bind(ip, port, context).await?;
        let token = CancellationToken::new();
        let shutdown = token.clone();

        server.start(token).await?;

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
        server.stop(shutdown).await?;

        Ok(())
    }
}
