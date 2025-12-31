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
