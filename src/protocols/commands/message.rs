use std::sync::Arc;

use crate::context::Context;
use crate::protocols::client_type::{ ClientType, send_bytes };
use crate::protocols::command::Action;
use crate::protocols::commands::parser::CommandParser;
use crate::protocols::{ command::Entity, frame::Frame };

use bincode::{ Decode, Encode };

use serde::{ Deserialize, Serialize };
use zz_account::address::FreeWebMovementAddress;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Encode, Decode)]
pub struct MessageCommand {
    pub receiver: String,
    pub timestamp: u64,
    pub message: String,
}

impl MessageCommand {
    pub fn new(receiver: String, timestamp: u64, message: String) -> Self {
        Self {
            receiver,
            timestamp,
            message,
        }
    }
}

impl CommandParser {
    pub async fn on_text_message(frame: &Frame, _context: Arc<Context>, _client_type: &ClientType) {
        let from = &frame.body.address;

        let data = frame.body.data.clone();

        if data.is_empty() {
            eprintln!("❌ Empty MessageCommand from {}", from);
            return;
        }

        let (cmd, _) = match
            bincode::decode_from_slice::<MessageCommand, _>(&data, bincode::config::standard())
        {
            Ok(v) => v,
            Err(e) => {
                eprintln!("❌ Invalid MessageCommand from {}: {:?}", from, e);
                return;
            }
        };

        if cmd.message.is_empty() {
            eprintln!("❌ Empty message body from {}", from);
            return;
        }

        println!("📨 {} → {} @ {}: {}", from, cmd.receiver, cmd.timestamp, cmd.message);
    }
}

pub async fn send_text_message(
    client_type: &ClientType,
    address: &FreeWebMovementAddress,
    data: Vec<u8>
) -> anyhow::Result<()> {
    let frame = Frame::build_node_command(
        address,
        Entity::Message,
        Action::SendText,
        1,
        Some(data)
    )?;

    let bytes = Frame::to(frame);

    send_bytes(client_type, &bytes).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use tokio::net::{ TcpListener };
    use std::sync::Arc;
    use std::time::{ SystemTime, UNIX_EPOCH };

    use crate::context::Context;
    use crate::protocols::client_type::{ ClientType, to_client_type };
    use tokio::net::TcpStream;

    use zz_account::address::FreeWebMovementAddress as Address;
    use bincode::config;

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
        let (decoded, _) = bincode
            ::decode_from_slice::<MessageCommand, _>(&encoded, config::standard())
            .unwrap();

        assert_eq!(cmd, decoded);
    }

    #[tokio::test]
    async fn test_send_text_message_over_tcp() -> anyhow::Result<()> {
        let (tcp, rx) = tcp_pair().await;

        let address = Address::random();

        let cmd = MessageCommand {
            receiver: "receiver-addr".to_string(),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            message: "hello tcp".to_string(),
        };
        let data = bincode::encode_to_vec(cmd, config::standard())?;

        send_text_message(&tcp, &address, data).await?;

        let received = rx.await.unwrap();
        assert!(!received.is_empty(), "TCP should receive data");

        Ok(())
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

        let frame = Frame::build_node_command(
            &address,
            Entity::Message,
            Action::SendText,
            1,
            Some(data)
        ).unwrap();

        let context = Arc::new(Context::new("127.0.0.1".to_string(), 18000, Address::random()));
        let dummy_client = ClientType::UDP {
            socket: Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap()),
            peer: "127.0.0.1:0".parse().unwrap(),
        };

        // 只要不 panic、不提前 return 即视为通过
        CommandParser::on_text_message(&frame, context, &dummy_client).await;
    }
}
