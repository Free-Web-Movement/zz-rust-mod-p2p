use std::sync::Arc;

use crate::protocols::client_type::{ClientType, send_bytes};
use crate::protocols::command::{Action, Command, Entity};
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
    context: Arc<crate::context::Context>,
    client_type: &ClientType,
    data: Option<Vec<u8>>,
) -> anyhow::Result<()> {
    let command = Command::new(Entity::Node, Action::OffLine, data);

    let frame = Frame::build(context, command, 1)
        .await
        .unwrap();
    // 2️⃣ 序列化 Frame
    let bytes = Frame::to(frame);
    send_bytes(&client_type, &bytes).await;
    // self.send(&bytes).await?;
    Ok(())
}
