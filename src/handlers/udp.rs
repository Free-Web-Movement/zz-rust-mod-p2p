use crate::{
    context::Context,
    protocols::{ client_type::{ ClientType, send_bytes }, defines::Listener },
};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::net::UdpSocket;

pub const PEEK_UDP_BUFFER_LENGTH: usize = 1024;

#[derive(Clone)]
pub struct UDPHandler {
    context: Arc<Context>,
    listener: Arc<UdpSocket>,
}

impl UDPHandler {
    pub async fn bind(context: Arc<Context>) -> anyhow::Result<Arc<Self>> {
        let udp_addr = format!("{}:{}", context.ip, context.port);
        let listener = Arc::new(UdpSocket::bind(&udp_addr).await?);

        println!("UDP listening on {}", udp_addr);

        Ok(Arc::new(Self { context, listener }))
    }

    pub async fn start(self: Arc<Self>) -> anyhow::Result<()> {
        let socket = self.listener.clone();
        let shutdown = self.context.token.clone();

        tokio::spawn(async move {
            let mut buf = vec![0u8; PEEK_UDP_BUFFER_LENGTH];

            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => {
                        println!("UDP listener shutting down");
                        break;
                    }

                    res = socket.recv_from(&mut buf) => {
                        match res {
                            Ok((n, src)) => {
                                // Use the received peer address directly to avoid converting between
                                // different SocketAddr types (std vs tokio::net::unix).
                                let peer = src;
                                let client_type = ClientType::UDP { socket: socket.clone(), peer };
                                println!("UDP received {} bytes from {}", n, peer);
                                send_bytes(&client_type, &buf[..n]).await;
                            }
                            Err(e) => {
                                eprintln!("UDP recv error: {:?}", e);
                                break;
                            }
                        }
                    }
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
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

    use crate::protocols::client_type::stop;

    use super::*;
    use tokio::net::UdpSocket;
    use zz_account::address::FreeWebMovementAddress;

    #[tokio::test]
    async fn test_udp_echo() -> anyhow::Result<()> {
        let ip = "127.0.0.1";
        let port = 19000;

        let address = FreeWebMovementAddress::random();
        let context = Arc::new(Context::new(ip.to_string(), port, address));

        let server = UDPHandler::bind(context).await?;
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

        let address = FreeWebMovementAddress::random();
        let context = Arc::new(Context::new(ip.to_string(), port, address));

        let context_clone = context.clone();

        let server = UDPHandler::bind(context).await?;
        let server_clone = Arc::clone(&server);

        server_clone.start().await?;

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

        let ipv4: Ipv4Addr = "127.0.0.1".parse().unwrap();
        let client_type = ClientType::UDP { 
          socket: Arc::new(client), 
          peer: SocketAddr::V4(SocketAddrV4::new(ipv4, port)),
        };
        let _ = stop(&client_type, &context_clone).await;
        // server.clone().stop().await?;

        Ok(())
    }
}
