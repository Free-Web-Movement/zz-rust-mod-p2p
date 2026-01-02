use std::sync::Arc;

use zz_account::address::FreeWebMovementAddress;

use crate::protocols::client_type::{ClientType, send_bytes};
use crate::protocols::command::{Action, Entity};
use crate::protocols::frame::Frame;


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
