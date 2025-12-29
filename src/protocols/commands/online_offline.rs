use std::sync::Arc;

use crate::context::Context;
use crate::nodes::servers::Servers;
use crate::protocols::client_type::{ClientType, send_bytes};
use crate::protocols::commands::parser::CommandParser;
use crate::protocols::commands::sender::CommandSender;
use crate::protocols::{ command::{ Entity, Action }, frame::Frame };
use zz_account::address::FreeWebMovementAddress;

impl CommandParser {
    pub async fn on_node_online(frame: &Frame, context: Arc<Context>, client_type: &ClientType) {
        println!("✅ Node Online: addr={}, nonce={}", frame.body.address, frame.body.nonce);

        // 3️⃣ data 至少要有 flag
        if frame.body.data.len() < 1 {
            eprintln!("❌ Online data too short");
            return;
        }

        // 4️⃣ 拆 data
        // let (endpoint_bytes, flag) = frame.body.data.split_at(frame.body.data.len() - 1);

        // let is_inner = flag[0] == 0;

        // 5️⃣ 解 endpoint 列表
        let (endpoints, is_inner) = match Servers::from_endpoints(frame.body.data.to_vec()) {
            (endpoints, flag) => (endpoints, flag == 0),
        };
        // 6️⃣ 只处理 TCP client
        // let tcp = match client_type {
        //     ClientType::TCP(tcp) => tcp.clone(),
        //     _ => {
        //         eprintln!("❌ Online command not from TCP");
        //         return;
        //     }
        // };

        // 7️⃣ 注册 client
        // for ep in endpoints {
        let addr = frame.body.address.clone();
        let mut clients = context.clients.lock().await;

        if is_inner {
            clients.add_inner(&addr, client_type.clone(), endpoints.clone());
        } else {
            clients.add_external(&addr, client_type.clone(), endpoints.clone());
        }
    }

    pub async fn on_node_offline(
        frame: &Frame,
        context: Arc<crate::context::Context>,
        client_type: &ClientType
    ) {
        // 处理 Node Offline 命令的逻辑
        println!(
            "Node Offline Command Received: addr={}, nonce={}",
            frame.body.address,
            frame.body.nonce
        );
        let addr = frame.body.address.clone();
        let mut clients = context.clients.lock().await;
        clients.remove_client(&addr);

        // 这里可以添加更多处理逻辑，比如注销节点、更新状态等
    }
}

impl CommandSender {
    pub async fn send_online(
        &self,
        address: &FreeWebMovementAddress,
        data: Option<Vec<u8>>
    ) -> anyhow::Result<()> {
        // 1️⃣ 构建在线命令 Frame
        let frame = Frame::build_node_command(address, Entity::Node, Action::OnLine, 1, data)?;

        // 2️⃣ 序列化 Frame
        let bytes = Frame::to(frame);

        send_bytes(&self.tcp, &bytes).await;
        // self.tcp.send(&bytes).await?;
        Ok(())
    }

    pub async fn send_offline(
        &self,
        address: &FreeWebMovementAddress,
        data: Option<Vec<u8>>
    ) -> anyhow::Result<()> {
        // 1️⃣ 构建在线命令 Frame
        let frame = Frame::build_node_command(address, Entity::Node, Action::OffLine, 1, data)?;

        // 2️⃣ 序列化 Frame
        let bytes = Frame::to(frame);
        send_bytes(&self.tcp, &bytes).await;
        // self.send(&bytes).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::protocols::client_type::to_client_type;

    use super::*;
    use tokio::net::{ TcpListener, TcpStream };
    use zz_account::address::FreeWebMovementAddress as Address;

    /// 辅助函数：创建 TCP client/server pair
    async fn tcp_pair() -> (ClientType, tokio::sync::oneshot::Receiver<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let (tx, rx) = tokio::sync::oneshot::channel();

        tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 1024];
            let n = socket.readable().await.unwrap();
            let n = socket.try_read(&mut buf).unwrap();
            let _ = tx.send(buf[..n].to_vec());
        });

        let client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();

        let tcp = to_client_type(client);
        (tcp, rx)
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
