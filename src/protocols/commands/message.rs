use std::sync::Arc;

use crate::protocols::command::{Action, Entity, P2PCommand};
use crate::protocols::frame::P2PFrame;
use aex::connection::context::Context;
use aex::tcp::types::Codec;
use aex::time::SystemTime;

use bincode::{Decode, Encode};

use serde::{Deserialize, Serialize};
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
    message: &str,
) -> anyhow::Result<()> {
    let command = MessageCommand {
        receiver: receiver.clone(),
        timestamp: SystemTime::timestamp(),
        message: message.to_string(),
    };

    let manager = {
        let guard = ctx.lock().await;
        guard.global.manager.clone()
    };

    manager
        .forward(|entries| async move {
            for entry in entries {
                {
                    if let Some(ctx) = &entry.context {
                        P2PFrame::send(
                            ctx.clone(),
                            &Some(command.clone()),
                            Entity::Message,
                            Action::SendText,
                            true
                        )
                        .await
                        .expect("Error Send Message!");
                    }
                }
            }
        })
        .await;

    Ok(())
}

pub async fn message_handler(ctx: Arc<Mutex<Context>>, frame: P2PFrame, cmd: P2PCommand) {
    let from = &frame.body.address;
    println!("get encrypted bytes: {:?}", cmd.data);
    let psk = {
        let guard = ctx.lock().await;
        guard.global.paired_session_keys.clone().unwrap()
    };

    let plaintext: Vec<u8> = {
        let guard = psk.lock().await;
        guard
            .decrypt(&from.as_bytes().to_vec(), &cmd.data)
            .await
            .expect("Wrong encrypted data!")
    };

    println!("get plain text: {:?}", plaintext);

    let message: MessageCommand = match Codec::decode(&plaintext) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("❌ Invalid MessageCommand from {}: {:?}", from, e);
            return;
        }
    };

    println!(
        "📨 {} → {} @ {}: {}",
        from, message.receiver, message.timestamp, message.message
    );

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

    manager
        .forward(|entries| async move {
            for entry in entries {
                {
                    if let Some(ctx) = &entry.context {
                    
                        P2PFrame::send(
                            ctx.clone(),
                            &Some(message.clone()),
                            Entity::Message,
                            Action::SendText,
                            true
                        )
                        .await
                        .expect("Error Send Message!");
                    }
                }
            }
        })
        .await

    // 如果是作为服务器接收的消息，即地址不是节点地址时，
    // 要转发消息

    // forward_frame(receiver, &frame, context).await;
}
