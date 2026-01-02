use std::sync::Arc;

use anyhow::{Result, anyhow};
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{
    context::Context,
    protocols::{
        client_type::{self, ClientType},
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
    pub fn to_bytes(&self) -> Vec<u8> {
        let addr_bytes = self.address.as_bytes();
        let addr_len = addr_bytes.len() as u16;

        let mut buf = Vec::with_capacity(16 + 2 + addr_bytes.len() + 32);

        // session_id
        buf.extend_from_slice(&self.session_id);

        // address length
        buf.extend_from_slice(&addr_len.to_be_bytes());

        // address utf-8 bytes
        buf.extend_from_slice(addr_bytes);

        // ephemeral public key
        buf.extend_from_slice(&self.ephemeral_public_key);

        buf
    }
}

impl OnlineAckCommand {
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        // 最小长度
        if data.len() < 16 + 2 + 32 {
            return Err(anyhow!("online ack command too short"));
        }

        let mut offset = 0;

        // session_id
        let session_id: [u8; 16] = data[offset..offset + 16]
            .try_into()
            .map_err(|_| anyhow!("invalid session_id"))?;
        offset += 16;

        // address length
        let addr_len = u16::from_be_bytes(data[offset..offset + 2].try_into().unwrap()) as usize;
        offset += 2;

        // 边界检查
        if data.len() < offset + addr_len + 32 {
            return Err(anyhow!("invalid address length"));
        }

        // address
        let address = std::str::from_utf8(&data[offset..offset + addr_len])
            .map_err(|_| anyhow!("address is not valid utf-8"))?
            .to_string();
        offset += addr_len;

        // ephemeral public key
        let ephemeral_public_key: [u8; 32] = data[offset..offset + 32]
            .try_into()
            .map_err(|_| anyhow!("invalid ephemeral public key"))?;

        Ok(Self {
            session_id,
            address,
            ephemeral_public_key,
        })
    }
}

pub async fn on_node_online_ack(frame: &Frame, context: Arc<Context>, client_type: &ClientType) {
    println!(
        "✅ Node OnlineAck: from={}, nonce={}",
        frame.body.address, frame.body.nonce
    );

    // ===== 1️⃣ 解码 OnlineAckCommand =====
    let ack = match OnlineAckCommand::from_bytes(&frame.body.data) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("❌ decode OnlineAckCommand failed: {e}");
            return;
        }
    };

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
        // ✅ session_keys 锁在这里释放
    }

    println!(
        "🔐 Session established with {} (session_id={:?})",
        ack.address, ack.session_id
    );
}
