use std::sync::Arc;

use crate::protocols::command::{Action, Entity, P2PCommand};
use crate::protocols::frame::P2PFrame;
use crate::{context::Context, protocols::frame::forward_frame};
use aex::tcp::types::Codec;
use aex::time::SystemTime;

use bincode::{Decode, Encode};

use futures::future::{BoxFuture, join_all};
use serde::{Deserialize, Serialize};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Encode, Decode)]
pub struct MessageCommand {
    pub receiver: String,
    pub timestamp: u128,
    pub message: String,
}

impl Codec for MessageCommand {}

// 本地Node向外发送
pub async fn send_text_message(
    receiver: String,
    context: Arc<Context>,
    message: &str,
) -> anyhow::Result<()> {
    let command = MessageCommand {
        receiver: receiver.clone(),
        timestamp: SystemTime::timestamp(),
        message: message.to_string(),
    };

    // 1️⃣ 尝试本地发送
    let clients = context.clients.lock().await;
    let local_conns = clients.get_connections(&receiver, true);

    println!(
        "Found {} local connections for {}",
        local_conns.len(),
        receiver
    );

    if !local_conns.is_empty() {
        let futures: Vec<_> = local_conns
            .into_iter()
            .map(|tcp_arc| {
                let address = context.address.clone();
                let command = command.clone();
                let psk = context.paired_session_keys.clone();
                println!("local tcp stream found.");
                tokio::spawn(async move {
                    let (_, writer) = tcp_arc;

                    let mut writer = &mut *writer.lock().await;

                    P2PFrame::send(
                        &address,
                        &mut writer,
                        &Some(command.clone()),
                        Entity::Message,
                        Action::SendText,
                        Some(psk),
                    )
                    .await
                    .expect("Error Send Message!");

                    // send_bytes(&tcp_arc, &bytes).await
                })
            })
            .collect();

        // 等待全部发送完成
        for f in futures {
            println!("sending!");
            let _ = f.await;
        }

        return Ok(());
    }

    // 2️⃣ 本地没有 -> 向所有已连接服务器发送
    {
        let servers = &context.clone().servers;
        let servers = servers.lock().await;
        if let Some(connected_servers) = &servers.connected_servers {
            // 使用 iter().chain() 合并两个列表
            let all_servers = connected_servers
                .inner
                .iter()
                .chain(connected_servers.external.iter());

            let futures = all_servers.map(|server| {
                // let bytes = bytes.clone();
                // let bytes = bytes.clone();
                let address = context.address.clone();
                let command = command.clone();
                let psk = context.paired_session_keys.clone();
                let client_type = server.client_type.clone();

                async move {
                    let (_, writer) = &client_type;
                    let mut guard = writer.lock().await;

                    P2PFrame::send(
                        &address,
                        &mut *guard,
                        &Some(command.clone()),
                        Entity::Message,
                        Action::SendText,
                        Some(psk),
                    )
                    .await
                    .expect("Error Send Message!");
                }
            });

            join_all(futures).await;
        }
    }

    Ok(())
}

pub fn on_text_message(
    cmd: P2PCommand,
    frame: P2PFrame,
    context: Arc<Context>,
    _writer: Arc<Mutex<OwnedWriteHalf>>,
) -> BoxFuture<'static, ()> {
    Box::pin(async move {
        let from = &frame.body.address;

        // let encrypted = &cmd.data;

        println!("get encrypted bytes: {:?}", cmd.data);

        // 1️⃣ 使用 session_key 解密

        let psk = context.paired_session_keys.clone();

        let guard = &mut *psk.lock().await;

        let plaintext: Vec<u8> = guard
            .decrypt(&from.as_bytes().to_vec(), &cmd.data)
            .await
            .expect("Wrong encrypted data!");

        println!("get plain text: {:?}", plaintext);

        // 2️⃣ bincode 解码
        let (msg, _) = match bincode::decode_from_slice::<MessageCommand, _>(
            &plaintext,
            bincode::config::standard(),
        ) {
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

        forward_frame(receiver, &frame, context).await;
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use bincode::config;
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
}
