use tokio::io::AsyncWriteExt;

use crate::protocols::defines::ClientType;

pub struct CommandSender;

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
