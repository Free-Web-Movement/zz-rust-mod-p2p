use anyhow::Result;
use anyhow::anyhow;
use std::sync::Arc;

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use zz_account::address::FreeWebMovementAddress;

use crate::context::Context;
use crate::nodes::servers::Servers;
use crate::protocols::client_type::{ClientType, send_bytes};
use crate::protocols::command::{Action, Entity};
use crate::protocols::commands::ack::OnlineAckCommand;
use crate::protocols::frame::Frame;

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
    frame: &Frame,
    context: Arc<Context>,
    client_type: &ClientType,
) -> Option<Frame> {
    println!(
        "✅ Node Online: addr={}, nonce={}",
        frame.body.address, frame.body.nonce
    );

    // ===== 1️⃣ OnlineCommand 解码 =====

    println!("Received data {:?}", frame.body.data);
    let online = match OnlineCommand::from_bytes(&frame.body.data) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("❌ decode OnlineCommand failed: {e}");
            return None;
        }
    };

    // ===== 2️⃣ session_id → temp session（⚠️ 限定作用域）=====
    let (server_pub, ack_frame) = {
        let mut temp_sessions = context.temp_sessions.lock().await;

        let session = match temp_sessions.get_mut(&online.session_id) {
            Some(s) => s,
            None => {
                eprintln!("❌ temp session not found: {:?}", online.session_id);
                return None;
            }
        };

        let peer_pub = x25519_dalek::PublicKey::from(online.ephemeral_public_key);
        if let Err(e) = session.establish(&peer_pub) {
            eprintln!("❌ session establish failed: {e}");
            return None;
        }

        session.touch();

        let ack = OnlineAckCommand {
            session_id: online.session_id,
            address: context.address.to_string(),
            ephemeral_public_key: *session.ephemeral_public.as_bytes(),
        };

        let frame = Frame::build_node_command(
            &context.address,
            Entity::Node,
            Action::OnLineAck,
            frame.body.version,
            Some(ack.to_bytes()),
        )
        .expect("build OnlineAck frame failed");

        // ✅ temp_sessions 锁在这里释放
        (ack.ephemeral_public_key, frame)
    };

    // ===== 3️⃣ clients 登记（⚠️ 也限定作用域）=====
    {
        let (endpoints, is_inner) = match Servers::from_endpoints(online.endpoints) {
            (endpoints, flag) => (endpoints, flag == 0),
        };

        let addr = frame.body.address.clone();
        let mut clients = context.clients.lock().await;

        if is_inner {
            clients.add_inner(&addr, client_type.clone(), endpoints);
        } else {
            clients.add_external(&addr, client_type.clone(), endpoints);
        }
        // ✅ clients 锁在这里释放
    }

    // ===== 4️⃣ 发送 ACK（此时无任何锁）=====
    send_bytes(client_type, &Frame::to(ack_frame.clone())).await;

    Some(ack_frame)
}

pub async fn send_online(
    client_type: &ClientType,
    address: &FreeWebMovementAddress,
    data: Option<Vec<u8>>,
) -> anyhow::Result<()> {
    let frame = Frame::build_node_command(
        &address, // 本节点地址
        Entity::Node,
        Action::OnLine, // 用 ResponseAddress 表示发送自身地址
        1,
        data,
    )?;
    let bytes = Frame::to(frame);

    send_bytes(client_type, &bytes).await;

    Ok(())
}
