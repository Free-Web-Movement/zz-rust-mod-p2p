use std::net::SocketAddr;

use crate::protocols::commands::sender::CommandSender;
use crate::protocols::defines::ClientType;
use crate::protocols::{
    command::{Entity, NodeAction},
    frame::Frame,
};
use zz_account::address::FreeWebMovementAddress;

impl CommandSender {
    pub async fn send_online_command(
        client: ClientType,
        address: &FreeWebMovementAddress,
        _target: SocketAddr,
        data: Option<Vec<u8>>,
    ) -> anyhow::Result<()> {
        // 1️⃣ 构建在线命令 Frame
        let frame = Frame::build_node_command(address, Entity::Node, NodeAction::OnLine, 1, data)?;

        // 2️⃣ 序列化 Frame
        let bytes = Frame::to(frame);
        CommandSender::send_client(client, &bytes).await?;
        Ok(())
    }

    pub async fn send_offline_command(
        client: ClientType,
        address: &FreeWebMovementAddress,
        _target: SocketAddr,
        data: Option<Vec<u8>>,
    ) -> anyhow::Result<()> {
        // 1️⃣ 构建在线命令 Frame
        let frame = Frame::build_node_command(address, Entity::Node, NodeAction::OffLine, 1, data)?;

        // 2️⃣ 序列化 Frame
        let bytes = Frame::to(frame);
        CommandSender::send_client(client, &bytes).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::io::AsyncReadExt;
    use tokio::sync::Mutex;

    fn test_address() -> FreeWebMovementAddress {
        FreeWebMovementAddress::random()
    }

    async fn tcp_pair() -> (tokio::net::TcpStream, tokio::net::TcpStream) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let client =
            tokio::spawn(async move { tokio::net::TcpStream::connect(addr).await.unwrap() });

        let (server, _) = listener.accept().await.unwrap();
        let client = client.await.unwrap();

        (client, server)
    }

    async fn udp_pair() -> (tokio::net::UdpSocket, tokio::net::UdpSocket) {
        let a = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let b = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        (a, b)
    }

    #[tokio::test]
    async fn test_send_client_tcp() {
        let (client, mut server) = tcp_pair().await;

        let client = ClientType::TCP(Arc::new(Mutex::new(client)));

        let data = b"hello";
        CommandSender::send_client(client, data).await.unwrap();

        let mut buf = [0u8; 5];
        server.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, data);
    }

    #[tokio::test]
    async fn test_send_client_http() {
        let (client, mut server) = tcp_pair().await;

        let client = ClientType::HTTP(Arc::new(Mutex::new(client)));

        let data = b"http";
        CommandSender::send_client(client, data).await.unwrap();

        let mut buf = [0u8; 4];
        server.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, data);
    }

    #[tokio::test]
    async fn test_send_client_ws() {
        let (client, mut server) = tcp_pair().await;

        let client = ClientType::WS(Arc::new(Mutex::new(client)));

        let data = b"ws";
        CommandSender::send_client(client, data).await.unwrap();

        let mut buf = [0u8; 2];
        server.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, data);
    }

    #[tokio::test]
    async fn test_send_client_udp() {
        let (a, b) = udp_pair().await;
        let peer = b.local_addr().unwrap();

        let client = ClientType::UDP {
            socket: Arc::new(a),
            peer,
        };

        let data = b"udp";

        CommandSender::send_client(client, data).await.unwrap();

        let mut buf = [0u8; 3];
        let (n, _) = b.recv_from(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], data);
    }

    #[tokio::test]
    async fn test_send_online_command() {
        let (client, mut server) = tcp_pair().await;

        let client = ClientType::TCP(Arc::new(Mutex::new(client)));
        let addr = test_address();

        CommandSender::send_online_command(
            client,
            &addr,
            "127.0.0.1:1234".parse().unwrap(),
            Some(b"data".to_vec()),
        )
        .await
        .unwrap();

        let mut buf = Vec::new();
        server.read_to_end(&mut buf).await.unwrap();
        assert!(!buf.is_empty());
    }

    #[tokio::test]
    async fn test_send_offline_command() {
        let (client, mut server) = tcp_pair().await;

        let client = ClientType::TCP(Arc::new(Mutex::new(client)));
        let addr = test_address();

        CommandSender::send_offline_command(client, &addr, "127.0.0.1:1234".parse().unwrap(), None)
            .await
            .unwrap();

        let mut buf = Vec::new();
        server.read_to_end(&mut buf).await.unwrap();
        assert!(!buf.is_empty());
    }
}
