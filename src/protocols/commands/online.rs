use anyhow::Result;
use anyhow::anyhow;
use std::sync::Arc;

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::context::Context;
use crate::nodes::servers::Servers;
use crate::protocols::client_type::{ClientType, send_bytes};
use crate::protocols::codec::Codec;
use crate::protocols::command::Command;
use crate::protocols::command::{Action, Entity};
use crate::protocols::commands::ack::OnlineAckCommand;
use crate::protocols::frame::Frame;
use crate::protocols::session_key::SessionKey;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Encode, Decode)]
pub struct OnlineCommand {
    pub session_id: [u8; 16], // 临时 session id
    pub endpoints: Vec<u8>,
    pub ephemeral_public_key: [u8; 32],
}

impl OnlineCommand {
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::encode_to_vec(self, bincode::config::standard()).unwrap()
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let (cmd, _): (Self, _) = bincode::decode_from_slice(data, bincode::config::standard())
            .map_err(|e| anyhow!("decode OnlineCommand failed: {e}"))?;
        Ok(cmd)
    }
}

pub async fn on_node_online(
    cmd: &Command,
    frame: &Frame,
    context: Arc<Context>,
    client_type: &ClientType,
) -> Option<Frame> {
    println!(
        "✅ Node Online: addr={}, nonce={}",
        frame.body.address, frame.body.nonce
    );

    // ===== 1️⃣ OnlineCommand 解码 =====
    let online_data = match &cmd.data {
        Some(d) => d,
        None => {
            eprintln!("❌ OnlineCommand missing data");
            return None;
        }
    };

    let online = match OnlineCommand::from_bytes(&online_data.to_vec()) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("❌ decode OnlineCommand failed: {e}");
            return None;
        }
    };

    println!("received session_id: {:?}", online.session_id);

    // ===== 2️⃣ 服务器生成 SessionKey（包含 ephemeral keypair）并派生对称密钥 =====
    let mut session_key = SessionKey::new();
    let ephemeral_public = session_key.ephemeral_public.clone();
    let client_pub = x25519_dalek::PublicKey::from(online.ephemeral_public_key);

    if let Err(e) = session_key.establish(&client_pub) {
        eprintln!("❌ Failed to establish session key: {e}");
        return None;
    } else {
        println!(
            "🔐 Session established with {} (session_id={:?})",
            frame.body.address, online.session_id
        );
    }
    session_key.touch();

    // 保存 session_key 到 session_keys，key 为客户端 address
    context
        .session_keys
        .lock()
        .await
        .insert(frame.body.address.clone(), session_key);

    // ===== 3️⃣ 构造 OnlineAckCommand =====
    let ack = OnlineAckCommand {
        session_id: online.session_id,
        address: context.address.to_string(),
        ephemeral_public_key: ephemeral_public.to_bytes(),
    };

    println!("send ack session_id : {:?}", ack.session_id);
    println!("send ack: {:?}", ack.to_bytes());

    let command = Command::new(Entity::Node, Action::OnLineAck, Some(ack.to_bytes()));

    let ack_frame = Frame::build(context.clone(), command, 1)
        .await
        .unwrap();

    // ===== 4️⃣ clients 登记 =====
    let (endpoints, is_inner) = match Servers::from_endpoints(online.endpoints) {
        (eps, flag) => (eps, flag == 0),
    };
    let addr = frame.body.address.clone();
    let mut clients = context.clients.lock().await;
    if is_inner {
        clients.add_inner(&addr, client_type.clone(), endpoints);
    } else {
        clients.add_external(&addr, client_type.clone(), endpoints);
    }

    // ===== 5️⃣ 发送 ACK =====
    send_bytes(client_type, &Frame::to_bytes(&ack_frame.clone())).await;

    Some(ack_frame)
}

pub async fn send_online(
    context: Arc<Context>,
    client_type: &ClientType,
    data: Option<Vec<u8>>,
) -> anyhow::Result<()> {
    let command = Command::new(Entity::Node, Action::OnLine, data);

    let frame = Frame::build(context, command, 1)
        .await
        .unwrap();

    let bytes = Frame::to_bytes(&frame);

    send_bytes(client_type, &bytes).await;

    Ok(())
}
