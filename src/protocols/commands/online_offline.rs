use std::net::SocketAddr;

use crate::protocols::commands::sender::CommandSender;
use crate::protocols::defines::ClientType;
use crate::protocols::{
    command::{Entity, NodeAction},
    frame::Frame,
};
use tokio::io::AsyncWriteExt;
use zz_account::address::FreeWebMovementAddress;

impl CommandSender {
    pub async fn send_client(&self, client: ClientType, data: &[u8]) -> anyhow::Result<()> {
        match client {
            ClientType::TCP(tcp) | ClientType::HTTP(tcp) | ClientType::WS(tcp) => {
                let sender = tcp.clone();
                let mut stream = sender.lock().await;
                stream.write_all(data).await?;
            }
            ClientType::UDP { socket, peer } => {
                let sender = socket.clone();
                sender.send_to(data, peer).await?;
            }
        }
        Ok(())
    }
    pub async fn send_online_command(
        &self,
        client: ClientType,
        address: &FreeWebMovementAddress,
        _target: SocketAddr,
        data: Option<Vec<u8>>,
    ) -> anyhow::Result<()> {
        // 1️⃣ 构建在线命令 Frame
        let frame = Frame::build_node_command(address, Entity::Node, NodeAction::OnLine, 1, data)?;

        // 2️⃣ 序列化 Frame
        let bytes = Frame::to(frame);
        self.send_client(client, &bytes).await?;
        Ok(())
    }

    pub async fn send_offline_command(
        &self,
        client: ClientType,
        address: &FreeWebMovementAddress,
        _target: SocketAddr,
        data: Option<Vec<u8>>,
    ) -> anyhow::Result<()> {
        // 1️⃣ 构建在线命令 Frame
        let frame = Frame::build_node_command(address, Entity::Node, NodeAction::OnLine, 1, data)?;

        // 2️⃣ 序列化 Frame
        let bytes = Frame::to(frame);
        self.send_client(client, &bytes).await?;
        Ok(())
    }
}
