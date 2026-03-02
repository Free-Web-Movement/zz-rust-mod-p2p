use std::sync::Arc;

use aex::tcp::types::Codec;
use bincode::{Decode, Encode};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use tokio::{net::tcp::OwnedWriteHalf, sync::Mutex};

use crate::{
    context::Context,
    protocols::{ command::P2PCommand, frame::P2PFrame},
};

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct OnlineAckCommand {
    pub session_id: Vec<u8>,            // 临时 session id
    pub address: String,                // ⚠️ 明确：String
    pub ephemeral_public_key: [u8; 32], // 对方 ephemeral 公钥
}

impl Codec for OnlineAckCommand {}

pub fn on_online_ack(
    cmd: P2PCommand,
    frame: P2PFrame,
    context: Arc<Context>,
    _writer: Arc<Mutex<OwnedWriteHalf>>,
) -> BoxFuture<'static, ()> {
    Box::pin(async move {
        println!(
            "✅ Node OnlineAck received from {} nonce={}",
            frame.body.address, frame.body.nonce
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

        let psk = context.paired_session_keys.clone();

        let guard = &mut *psk.lock().await;

        guard
            .establish_ends(
                ack.address.as_bytes().to_vec(),
                &ack.ephemeral_public_key.to_vec(),
            )
            .await
            .expect("!!");

        println!(
            "🔐 Session established with {} (session_id={:?})",
            ack.address, ack.session_id
        );
    })
}
