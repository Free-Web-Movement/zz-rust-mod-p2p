use std::sync::Arc;

use anyhow::{Result, anyhow};
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use zz_account::address::FreeWebMovementAddress;

use crate::{
    context::Context,
    protocols::{
        client_type::{ClientType, send_bytes},
        command::{Action, Command, Entity},
        frame::Frame,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct OnlineAckCommand {
    pub session_id: [u8; 16],           // 临时 session id
    pub address: String,                // ⚠️ 明确：String
    pub ephemeral_public_key: [u8; 32], // 对方 ephemeral 公钥
}

impl OnlineAckCommand {
    // 使用 bincode 2.0 序列化
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::encode_to_vec(self, bincode::config::standard()).unwrap()
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let (cmd, _): (Self, _) = bincode::decode_from_slice(data, bincode::config::standard())
            .map_err(|e| anyhow!("decode OnlineAckCommand failed: {e}"))?;
        Ok(cmd)
    }
}

pub async fn on_node_online_ack(
    cmd: &Command,
    frame: &Frame,
    context: Arc<Context>,
    _client_type: &ClientType, // 这里暂时不需要，因为我们只处理 temp_sessions
) {
    println!(
        "✅ Node OnlineAck received from {} nonce={}",
        frame.body.address, frame.body.nonce
    );

    println!("received ack: {:?}", cmd.data.as_ref().unwrap());
    // ===== 1️⃣ 解码 OnlineAckCommand =====
    let ack = match OnlineAckCommand::from_bytes(&cmd.data.as_ref().unwrap()) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("❌ decode OnlineAckCommand failed: {e}");
            return;
        }
    };

    println!("session:id: {:?}", ack.session_id);

    // ===== 2️⃣ 从 temp_sessions 中取出 session（限定作用域）=====
    let session = {
        let mut temp_sessions = context.temp_sessions.lock().await;

        let mut session = match temp_sessions.remove(&ack.session_id) {
            Some(s) => s,
            None => {
                eprintln!(
                    "❌ temp session not found for session_id={:?}",
                    ack.session_id
                );
                return;
            }
        };

        let peer_pub = x25519_dalek::PublicKey::from(ack.ephemeral_public_key);
        if let Err(e) = session.establish(&peer_pub) {
            eprintln!("❌ session establish failed: {e}");
            return;
        }

        session.touch();
        session
        // ✅ temp_sessions 锁在这里释放
    };

    // ===== 3️⃣ 写入永久 session_keys（address → session）=====
    {
        let mut sessions = context.session_keys.lock().await;
        sessions.insert(ack.address.clone(), session);
    }

    println!(
        "🔐 Session established with {} (session_id={:?})",
        ack.address, ack.session_id
    );
}

pub async fn send_online_ack(
    client_type: &ClientType,
    address: &FreeWebMovementAddress,
    ack: OnlineAckCommand, // 传入已经构造好的 OnlineAckCommand
) -> Result<()> {
    // 1️⃣ 构造 Frame
    let frame = Frame::build_node_command(
        address,              // 本节点地址
        Entity::Node,         // 节点命令
        Action::OnLineAck,    // ACK 动作
        1,                    // version
        Some(ack.to_bytes()), // 序列化数据
    )?;

    // 2️⃣ 转成字节发送
    let bytes = Frame::to(frame);

    send_bytes(client_type, &bytes).await;

    Ok(())
}
