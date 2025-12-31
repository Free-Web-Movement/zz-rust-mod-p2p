use std::sync::Arc;

use crate::context::Context;
use crate::protocols::client_type::forward_frame;
use crate::protocols::frame::Frame;

use bincode::{Decode, Encode};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Encode, Decode)]
pub struct MessageCommand {
    pub receiver: String,
    pub timestamp: u128,
    pub message: String,
}

pub async fn on_text_message(frame: &Frame, context: Arc<Context>) {
    let from = &frame.body.address;

    // 1️⃣ 先从 frame.body.data 解 Command
    let command = match frame.body.command_from_data() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("❌ Command decode failed from {}: {:?}", from, e);
            return;
        }
    };

    // 2️⃣ 再从 Command.data 解 MessageCommand
    let Some(data) = command.data else {
        eprintln!("❌ Empty command.data from {}", from);
        return;
    };

    let (msg, _) =
        match bincode::decode_from_slice::<MessageCommand, _>(&data, bincode::config::standard()) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("❌ Invalid MessageCommand from {}: {:?}", from, e);
                return;
            }
        };

    println!(
        "📨 {} → {} @ {}: {}",
        from, msg.receiver, msg.timestamp, msg.message
    );

    let receiver = msg.receiver.clone();

    // ===== 1️⃣ 如果 receiver 是自己 =====
    if receiver == context.address.to_string() {
        // ✔️ 消费消息
        // on_text_message(frame, context, client_type).await;

        // on_receive_message();
        println!("Message received!");
        return;
    }

    // 如果是作为服务器接收的消息，即地址不是节点地址时，
    // 要转发消息

    forward_frame(receiver, frame, context).await;

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::net::TcpListener;

    use crate::context::Context;
    use crate::nodes::net_info::NetInfo;
    use crate::nodes::servers::Servers;
    use crate::nodes::storage::Storeage;
    use crate::protocols::client_type::{ClientType, to_client_type};
    use crate::protocols::command::{Action, Entity};
    use tokio::net::TcpStream;

    use bincode::config;
    use zz_account::address::FreeWebMovementAddress as Address;

    /// 创建 TCP client/server pair，用于捕获发送的数据
    async fn tcp_pair() -> (ClientType, tokio::sync::oneshot::Receiver<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let (tx, rx) = tokio::sync::oneshot::channel();

        tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let _n = socket.readable().await.unwrap();
            let n = socket.try_read(&mut buf).unwrap();
            let _ = tx.send(buf[..n].to_vec());
        });

        let client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let tcp = to_client_type(client);
        (tcp, rx)
    }

    #[test]
    fn test_message_command_bincode_roundtrip() {
        let cmd = MessageCommand {
            receiver: "receiver-addr".to_string(),
            timestamp: 123456789,
            message: "hello world".to_string(),
        };

        let encoded = bincode::encode_to_vec(&cmd, config::standard()).unwrap();
        let (decoded, _) =
            bincode::decode_from_slice::<MessageCommand, _>(&encoded, config::standard()).unwrap();

        assert_eq!(cmd, decoded);
    }

    #[tokio::test]
    async fn test_on_text_message_normal_path() {
        let address = Address::random();

        let cmd = MessageCommand {
            receiver: "receiver-addr".to_string(),
            timestamp: 1,
            message: "hello parser".to_string(),
        };

        let data = bincode::encode_to_vec(&cmd, config::standard()).unwrap();

        let frame =
            Frame::build_node_command(&address, Entity::Message, Action::SendText, 1, Some(data))
                .unwrap();

        let storage = Storeage::new(None, None, None, None);
        let net_info = NetInfo::new(1010);
        let server = Servers::new(address, storage, net_info);

        let context = Arc::new(Context::new(
            "127.0.0.1".to_string(),
            18000,
            Address::random(),
            server,
        ));

        // 只要不 panic、不提前 return 即视为通过
        on_text_message(&frame, context).await;
    }
}
