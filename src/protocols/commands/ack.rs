use std::sync::Arc;

use aex::{ connection::context::Context, tcp::types::Codec };
use bincode::{ Decode, Encode };
use serde::{ Deserialize, Serialize };
use tokio::sync::Mutex;

use crate::protocols::{ command::P2PCommand, frame::P2PFrame };

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct OnlineAckCommand {
    pub session_id: Vec<u8>, // 临时 session id
    pub address: String, // ⚠️ 明确：String
    pub ephemeral_public_key: [u8; 32], // 对方 ephemeral 公钥
}

impl Codec for OnlineAckCommand {}

pub async fn onlineack_handler(ctx: Arc<Mutex<Context>>, frame: P2PFrame, cmd: P2PCommand) {
    println!("✅ Node OnlineAck received from {} nonce={}", frame.body.address, frame.body.nonce);

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

    let psk = {
        let guard = ctx.lock().await;
        guard.global.paired_session_keys.clone().unwrap()
    };

    {
        let guard = psk.lock().await;

        let _ = guard.establish_ends(
            ack.address.as_bytes().to_vec(),
            &ack.ephemeral_public_key.to_vec()
        ).await;
    }
    println!("🔐 Session established with {} (session_id={:?})", ack.address, ack.session_id);
}
