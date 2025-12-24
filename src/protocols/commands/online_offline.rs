
use crate::protocols::commands::sender::CommandSender;
use crate::protocols::{
    command::{Entity, NodeAction},
    frame::Frame,
};
use zz_account::address::FreeWebMovementAddress;

impl CommandSender {
    pub async fn send_online(
        &self,
        address: &FreeWebMovementAddress,
        data: Option<Vec<u8>>,
    ) -> anyhow::Result<()> {
        // 1️⃣ 构建在线命令 Frame
        let frame = Frame::build_node_command(address, Entity::Node, NodeAction::OnLine, 1, data)?;

        // 2️⃣ 序列化 Frame
        let bytes = Frame::to(frame);
        self.send(&bytes).await?;
        Ok(())
    }

    pub async fn send_offline(
        &self,
        address: &FreeWebMovementAddress,
        data: Option<Vec<u8>>,
    ) -> anyhow::Result<()> {
        // 1️⃣ 构建在线命令 Frame
        let frame = Frame::build_node_command(address, Entity::Node, NodeAction::OffLine, 1, data)?;

        // 2️⃣ 序列化 Frame
        let bytes = Frame::to(frame);
        self.send(&bytes).await?;
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::Mutex;
    use std::sync::Arc;
    use std::net::{IpAddr, Ipv4Addr};
    use crate::protocols::defines::ClientType;
    use zz_account::address::FreeWebMovementAddress as Address;

    /// 辅助函数：创建 TCP client/server pair
    async fn tcp_pair() -> (ClientType, tokio::sync::oneshot::Receiver<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let (tx, rx) = tokio::sync::oneshot::channel();

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 1024];
            let n = socket.readable().await.unwrap();
            let n = socket.try_read(&mut buf).unwrap();
            let _ = tx.send(buf[..n].to_vec());
        });

        let client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let client = Arc::new(Mutex::new(client));

        (ClientType::TCP(client), rx)
    }

    #[tokio::test]
    async fn test_send_online() -> anyhow::Result<()> {
        let (tcp, rx) = tcp_pair().await;

        let sender = CommandSender {
            tcp,
            udp: None,
        };

        let address = Address::random();
        let payload = Some(b"online-data".to_vec());

        sender.send_online(&address, payload).await?;

        // 验证 TCP 收到数据
        let received = rx.await.unwrap();
        assert!(!received.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_send_offline() -> anyhow::Result<()> {
        let (tcp, rx) = tcp_pair().await;

        let sender = CommandSender {
            tcp,
            udp: None,
        };

        let address = Address::random();
        let payload = Some(b"offline-data".to_vec());

        sender.send_offline(&address, payload).await?;

        // 验证 TCP 收到数据
        let received = rx.await.unwrap();
        assert!(!received.is_empty());

        Ok(())
    }
}

