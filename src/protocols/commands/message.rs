use std::sync::Arc;

use crate::protocols::command::{ Action, Entity, P2PCommand };
use crate::protocols::frame::P2PFrame;
use aex::connection::context::Context;
use aex::tcp::types::Codec;
use aex::time::SystemTime;

use bincode::{ Decode, Encode };

use serde::{ Deserialize, Serialize };
use tokio::sync::Mutex;
use zz_account::address::FreeWebMovementAddress;

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
    ctx: Arc<Mutex<Context>>,
    message: &str
) -> anyhow::Result<()> {
    let command = MessageCommand {
        receiver: receiver.clone(),
        timestamp: SystemTime::timestamp(),
        message: message.to_string(),
    };

    let address = {
        let guard = ctx.lock().await;
        let opt_address: Option<FreeWebMovementAddress> = guard
            .get().await
            .expect("FreeWebMovementAddress must be set!");
        opt_address.unwrap()
    };
    let psk = {
        let guard = ctx.lock().await;
        let global = guard.global.clone();
        global.paired_session_keys.clone()
    };

    let manager = {
        let guard = ctx.lock().await;
        guard.global.manager.clone()
    };

    manager.forward(|entries| async move {
        for entry in entries {
            {
                if let Some(writer_arc) = &entry.writer {
                    let writer_lock = writer_arc.clone();
                    let mut writer = writer_lock.lock().await;
                    P2PFrame::send(
                        &address,
                        &mut *writer,
                        &Some(command.clone()),
                        Entity::Message,
                        Action::SendText,
                        psk.clone()
                    ).await.expect("Error Send Message!");
                }
            }
        }
    }).await;

    Ok(())
}

// pub fn on_text_message(
//     cmd: P2PCommand,
//     frame: P2PFrame,
//     context: Arc<Mutex<Context>>,
//     _writer: Arc<Mutex<OwnedWriteHalf>>
// ) -> BoxFuture<'static, ()> {
//     Box::pin(async move {
//         let from = &frame.body.address;

//         // let encrypted = &cmd.data;

//         println!("get encrypted bytes: {:?}", cmd.data);

//         // 1️⃣ 使用 session_key 解密

//         let psk = context.paired_session_keys.clone();

//         let guard = &mut *psk.lock().await;

//         let plaintext: Vec<u8> = guard
//             .decrypt(&from.as_bytes().to_vec(), &cmd.data).await
//             .expect("Wrong encrypted data!");

//         println!("get plain text: {:?}", plaintext);

//         // 2️⃣ bincode 解码
//         let (msg, _) = match
//             bincode::decode_from_slice::<MessageCommand, _>(&plaintext, bincode::config::standard())
//         {
//             Ok(v) => v,
//             Err(e) => {
//                 eprintln!("❌ Invalid MessageCommand from {}: {:?}", from, e);
//                 return;
//             }
//         };

//         println!("📨 {} → {} @ {}: {}", from, msg.receiver, msg.timestamp, msg.message);

//         let receiver = msg.receiver.clone();

//         // ===== 1️⃣ 如果 receiver 是自己 =====
//         if receiver == context.address.to_string() {
//             // ✔️ 消费消息
//             // on_text_message(frame, context, client_type).await;

//             // on_receive_message();
//             println!("Message received!");
//             return;
//         }

//         // 如果是作为服务器接收的消息，即地址不是节点地址时，
//         // 要转发消息

//         forward_frame(receiver, &frame, context).await;
//     })
// }

pub async fn message_handler(ctx: Arc<Mutex<Context>>, frame: P2PFrame, cmd: P2PCommand) {
    let from = &frame.body.address;

    // let encrypted = &cmd.data;

    println!("get encrypted bytes: {:?}", cmd.data);

    // 1️⃣ 使用 session_key 解密

    let psk = {
        let guard = ctx.lock().await;
        guard.global.paired_session_keys.clone().unwrap()
    };

    let plaintext: Vec<u8> = {
        let guard = psk.lock().await;
        guard.decrypt(&from.as_bytes().to_vec(), &cmd.data).await.expect("Wrong encrypted data!")
    };

    println!("get plain text: {:?}", plaintext);

    let message: MessageCommand = match Codec::decode(&plaintext) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("❌ Invalid MessageCommand from {}: {:?}", from, e);
            return;
        }
    };

    println!("📨 {} → {} @ {}: {}", from, message.receiver, message.timestamp, message.message);

    let receiver = message.receiver.clone();

    // ===== 1️⃣ 如果 receiver 是自己 =====

    let address: FreeWebMovementAddress = {
        let guard = ctx.lock().await;
        guard.get().await.unwrap()
    };
    if receiver == address.to_string() {
        // ✔️ 消费消息
        // on_text_message(frame, context, client_type).await;

        // on_receive_message();
        println!("Message received!");
        return;
    }

    let manager = {
        let guard = ctx.lock().await;
        guard.global.manager.clone()
    };


    manager.forward(|entries| async move {
      for entry in entries {
            {
                if let Some(writer_arc) = &entry.writer {
                    let writer_lock = writer_arc.clone();
                    let mut writer = writer_lock.lock().await;
                    P2PFrame::send(
                        &address,
                        &mut *writer,
                        &Some(message.clone()),
                        Entity::Message,
                        Action::SendText,
                        Some(psk.clone())
                    ).await.expect("Error Send Message!");
                }
            }
      }
    }).await

    // 如果是作为服务器接收的消息，即地址不是节点地址时，
    // 要转发消息

    // forward_frame(receiver, &frame, context).await;
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
        let (decoded, _) = bincode
            ::decode_from_slice::<MessageCommand, _>(&encoded, config::standard())
            .unwrap();

        assert_eq!(cmd, decoded);
    }
}
