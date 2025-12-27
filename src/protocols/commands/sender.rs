use tokio::io::AsyncWriteExt;

use crate::protocols::defines::ClientType;
use anyhow::{anyhow, Result};
/* =========================
   CommandSender
========================= */

#[derive(Clone)]
pub struct CommandSender {
    /// 控制通道（必须存在，TCP / HTTP / WS 之一）
    pub tcp: ClientType,

    /// 数据通道（可选，UDP 优先）
    pub udp: Option<ClientType>,
}

impl CommandSender {
pub async fn send_client(client: ClientType, data: &[u8]) -> Result<()> {
    match client {
        ClientType::TCP(tcp)
        | ClientType::HTTP(tcp)
        | ClientType::WS(tcp) => {
            let mut guard = tcp.lock().await;

            match guard.as_mut() {
                Some(stream) => {
                    stream.write_all(data).await?;
                }
                None => {
                    return Err(anyhow!("TCP/HTTP/WS stream already closed"));
                }
            }
        }

        ClientType::UDP { socket, peer } => {
            socket.send_to(data, peer).await?;
        }
    }

    Ok(())
}

    pub async fn send(&self, data: &[u8]) -> anyhow::Result<()> {
        // UDP 优先
        if let Some(udp) = &self.udp {
            if Self::send_client(udp.clone(), data).await.is_ok() {
                return Ok(());
            }
        }

        // TCP fallback
        Self::send_client(self.tcp.clone(), data).await
    }
}

/* =========================
   Tests
========================= */

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use tokio::{
        io::AsyncReadExt,
        net::{TcpListener, TcpStream, UdpSocket},
        sync::{Mutex, oneshot},
        time::{Duration, timeout},
    };

    const TIMEOUT: Duration = Duration::from_secs(2);

    async fn tcp_pair() -> (ClientType, oneshot::Receiver<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let (tx, rx) = oneshot::channel();

        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 1024];
            let n = timeout(TIMEOUT, sock.read(&mut buf))
                .await
                .unwrap()
                .unwrap();
            let _ = tx.send(buf[..n].to_vec());
        });

        let stream = TcpStream::connect(addr).await.unwrap();
        let client = ClientType::TCP(Arc::new(Mutex::new(Some(stream))));

        (client, rx)
    }

    #[tokio::test]
    async fn test_send_client_tcp() {
        let (client, rx) = tcp_pair().await;
        CommandSender::send_client(client, b"tcp").await.unwrap();
        assert_eq!(rx.await.unwrap(), b"tcp");
    }

    #[tokio::test]
    async fn test_send_client_http() {
        let (client, rx) = tcp_pair().await;
        let http = match client {
            ClientType::TCP(t) => ClientType::HTTP(t),
            _ => unreachable!(),
        };
        CommandSender::send_client(http, b"http").await.unwrap();
        assert_eq!(rx.await.unwrap(), b"http");
    }

    #[tokio::test]
    async fn test_send_client_ws() {
        let (client, rx) = tcp_pair().await;
        let ws = match client {
            ClientType::TCP(t) => ClientType::WS(t),
            _ => unreachable!(),
        };
        CommandSender::send_client(ws, b"ws").await.unwrap();
        assert_eq!(rx.await.unwrap(), b"ws");
    }

    #[tokio::test]
    async fn test_send_client_udp() {
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let peer = server.local_addr().unwrap();

        let client = ClientType::UDP {
            socket: Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap()),
            peer,
        };

        CommandSender::send_client(client, b"udp").await.unwrap();

        let mut buf = [0u8; 16];
        let (n, _) = timeout(TIMEOUT, server.recv_from(&mut buf))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(&buf[..n], b"udp");
    }

    #[tokio::test]
    async fn test_send_udp_preferred() {
        let udp_server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let peer = udp_server.local_addr().unwrap();

        let udp = ClientType::UDP {
            socket: Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap()),
            peer,
        };

        let (tcp, mut rx) = tcp_pair().await;

        let sender = CommandSender {
            tcp,
            udp: Some(udp),
        };

        sender.send(b"hello").await.unwrap();

        let mut buf = [0u8; 16];
        let (n, _) = timeout(TIMEOUT, udp_server.recv_from(&mut buf))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(&buf[..n], b"hello");
        assert!(rx.try_recv().is_err());
    }

    // #[tokio::test]
    // async fn test_send_udp_fail_fallback_tcp() {
    //     let (tcp, mut rx) = tcp_pair().await;

    //     let bad_udp = ClientType::UDP {
    //         socket: Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap()),
    //         peer: "127.0.0.1:9".parse().unwrap(), // discard port
    //     };

    //     let sender = CommandSender {
    //         tcp,
    //         udp: Some(bad_udp),
    //     };

    //     sender.send(b"fallback").await.unwrap();
    //     assert_eq!(rx.await.unwrap(), b"fallback");
    // }

    #[tokio::test]
    async fn test_send_no_udp_direct_tcp() {
        let (tcp, rx) = tcp_pair().await;

        let sender = CommandSender {
            tcp,
            udp: None,
        };

        sender.send(b"direct").await.unwrap();
        assert_eq!(rx.await.unwrap(), b"direct");
    }
}
