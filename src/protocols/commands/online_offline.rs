use std::sync::Arc;

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use zz_account::address::FreeWebMovementAddress;

use crate::context::Context;
use crate::nodes::servers::Servers;
use crate::protocols::client_type::{ClientType, send_bytes};
use crate::protocols::command::{Action, Entity};
use crate::protocols::frame::Frame;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Encode, Decode)]
pub struct OnlineCommand {
    pub session_id: [u8; 16], // 临时 session id
    pub endpoints: Vec<u8>,
    pub ephemeral_public_key: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct OnlineAckCommand {
    pub session_id: [u8; 16],           // 对应 send_online 的 session_id
    pub ephemeral_public_key: [u8; 32], // 对方 ephemeral 公钥
}

pub async fn on_node_online(frame: &Frame, context: Arc<Context>, client_type: &ClientType) {
    println!(
        "✅ Node Online: addr={}, nonce={}",
        frame.body.address, frame.body.nonce
    );
    if frame.body.data.len() < 1 {
        eprintln!("❌ Online data too short");
        return;
    }

    let (endpoints, is_inner) = match Servers::from_endpoints(frame.body.data.to_vec()) {
        (endpoints, flag) => (endpoints, flag == 0),
    };

    let addr = frame.body.address.clone();
    let mut clients = context.clients.lock().await;

    if is_inner {
        clients.add_inner(&addr, client_type.clone(), endpoints.clone());
    } else {
        clients.add_external(&addr, client_type.clone(), endpoints.clone());
    }
}

pub async fn on_node_offline(
    frame: &Frame,
    context: Arc<crate::context::Context>,
    client_type: &ClientType,
) {
    // 处理 Node Offline 命令的逻辑
    println!(
        "Node Offline Command Received: addr={}, nonce={}",
        frame.body.address, frame.body.nonce
    );
    let addr = frame.body.address.clone();
    let mut clients = context.clients.lock().await;
    clients.remove_client(&addr).await;

    // 这里可以添加更多处理逻辑，比如注销节点、更新状态等
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

pub async fn send_offline(
    client_type: &ClientType,
    address: &FreeWebMovementAddress,
    data: Option<Vec<u8>>,
) -> anyhow::Result<()> {
    // 1️⃣ 构建在线命令 Frame
    let frame = Frame::build_node_command(address, Entity::Node, Action::OffLine, 1, data)?;

    // 2️⃣ 序列化 Frame
    let bytes = Frame::to(frame);
    send_bytes(&client_type, &bytes).await;
    // self.send(&bytes).await?;
    Ok(())
}
