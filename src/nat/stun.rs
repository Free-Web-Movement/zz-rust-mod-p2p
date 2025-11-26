use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::Notify;

/* =========================
   STUN SERVER
========================= */

pub struct StunServer {
    socket: UdpSocket,
    shutdown: Arc<Notify>,
}

impl StunServer {
    pub async fn new(ip: &str, port: u16) -> anyhow::Result<(Self, Arc<Notify>)> {
        let socket = UdpSocket::bind(format!("{ip}:{port}")).await?;
        let notify = Arc::new(Notify::new());

        Ok((
            Self {
                socket,
                shutdown: notify.clone(),
            },
            notify,
        ))
    }

    pub async fn start(&mut self) -> anyhow::Result<()> {
        let mut buf = [0u8; 1024];

        loop {
            tokio::select! {
                _ = self.shutdown.notified() => {
                    break;
                }

                result = self.socket.recv_from(&mut buf) => {
                    let (_, client) = result?;
                    let reply = format!("STUN_RESPONSE:{}", client);
                    self.socket.send_to(reply.as_bytes(), client).await?;
                }
            }
        }

        Ok(())
    }
}

/* =========================
   TESTS
========================= */

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{Duration, sleep};

    #[tokio::test]
    async fn test_stun_server_response() {
        let (mut stun, shutdown) = StunServer::new("127.0.0.1", 45000).await.unwrap();

        let server = tokio::spawn(async move {
            stun.start().await.unwrap();
        });

        sleep(Duration::from_millis(100)).await;

        let client = UdpSocket::bind("127.0.0.1:45001").await.unwrap();
        client.send_to(b"ping", "127.0.0.1:45000").await.unwrap();

        let mut buf = [0u8; 1024];
        let (len, _) = client.recv_from(&mut buf).await.unwrap();

        let msg = String::from_utf8_lossy(&buf[..len]);
        assert!(msg.starts_with("STUN_RESPONSE"));

        shutdown.notify_one();
        server.await.unwrap();
    }
}
