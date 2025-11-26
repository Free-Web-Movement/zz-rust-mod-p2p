use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::Notify;

/* =========================
   TURN SERVER
========================= */

pub struct TurnServer {
    socket: UdpSocket,
    shutdown: Arc<Notify>,
}

impl TurnServer {
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
        let mut buf = [0u8; 1500];

        loop {
            tokio::select! {
                _ = self.shutdown.notified() => break,
                res = self.socket.recv_from(&mut buf) => {
                    let (len, addr) = res?;
                    self.socket.send_to(&buf[..len], addr).await?;
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
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_turn_echo() {
        let (mut turn, shutdown) = TurnServer::new("127.0.0.1", 46000).await.unwrap();

        let server = tokio::spawn(async move {
            turn.start().await.unwrap();
        });

        sleep(Duration::from_millis(100)).await;

        let client = UdpSocket::bind("127.0.0.1:46001").await.unwrap();
        client.send_to(b"hello", "127.0.0.1:46000").await.unwrap();

        let mut buf = [0u8; 1500];
        let (len, _) = client.recv_from(&mut buf).await.unwrap();

        assert_eq!(&buf[..len], b"hello");

        shutdown.notify_one();
        server.await.unwrap();
    }
}
