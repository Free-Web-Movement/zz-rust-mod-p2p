use tokio::io::AsyncWriteExt;

use crate::protocols::defines::ClientType;

pub struct CommandSender {
    
    /// 控制通道（必须存在，TCP / HTTP / WS 之一）
    pub tcp: ClientType,

    /// 数据通道（可选，UDP 优先）
    pub udp: Option<ClientType>,
}

impl CommandSender {
    pub async fn send_client(client: ClientType, data: &[u8]) -> anyhow::Result<()> {
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
}
