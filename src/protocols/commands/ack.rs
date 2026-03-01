use std::sync::Arc;

use aex::tcp::types::Codec;
use anyhow::Result;
use bincode::{ Decode, Encode };
use futures::future::BoxFuture;
use serde::{ Deserialize, Serialize };

use crate::{
    context::Context,
    protocols::{
        client_type::{ ClientType, send_bytes },
        command::{ Action, P2PCommand, Entity }, frame::P2PFrame,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct OnlineAckCommand {
    pub session_id: Vec<u8>, // 临时 session id
    pub address: String, // ⚠️ 明确：String
    pub ephemeral_public_key: [u8; 32], // 对方 ephemeral 公钥
}

impl Codec for OnlineAckCommand {}

// pub async fn send_online_ack(
//     context: Arc<Context>,
//     client_type: &ClientType,
//     ack: OnlineAckCommand // 传入已经构造好的 OnlineAckCommand
// ) -> Result<()> {
//     let command = P2PCommand::new(Entity::Node as u8, Action::OnLineAck as u8, Codec::encode(&ack));

//     let frame = P2PFrame::build(&context.address, command, 1).await.unwrap();

//     // 2️⃣ 转成字节发送
//     let bytes = Codec::encode(&frame);

//     send_bytes(client_type, &bytes).await;

//     Ok(())
// }

pub fn on_online_ack(
    cmd: P2PCommand,
    frame: P2PFrame,
    context: Arc<Context>,
    _client_type: Arc<ClientType>
) -> BoxFuture<'static, ()> 
 {
    Box::pin(async move {
        println!(
            "✅ Node OnlineAck received from {} nonce={}",
            frame.body.address,
            frame.body.nonce
        );

        println!("received ack: {:?}", cmd.data);
        // ===== 1️⃣ 解码 OnlineAckCommand =====
        let ack: OnlineAckCommand = match Codec::decode(&cmd.data) {
            Ok(cmd) => cmd,
            Err(e) => {
                eprintln!("❌ decode OnlineAckCommand failed: {e}");
                return;
            }
        };

        println!("session:id: {:?}", ack.session_id);

        context.paired_session_keys.establish_ends(ack.address.as_bytes().to_vec(), &ack.ephemeral_public_key.to_vec()).await.expect("!!");

        println!("🔐 Session established with {} (session_id={:?})", ack.address, ack.session_id);
    })
}
