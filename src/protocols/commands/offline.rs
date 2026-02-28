use std::sync::Arc;

use aex::tcp::types::Codec;
use futures::future::BoxFuture;

use crate::context::Context;
use crate::protocols::client_type::{ ClientType, send_bytes };
use crate::protocols::command::{ Action, P2PCommand, Entity };
use crate::protocols::frame::P2PFrame;

pub fn on_offline(
    cmd: P2PCommand,
    frame: P2PFrame,
    context: Arc<Context>,
    client_type: Arc<ClientType>
) -> BoxFuture<'static, ()> {
    Box::pin(async move {
        // 处理 Node Offline 命令的逻辑
        println!(
            "Node Offline Command Received: addr={}, nonce={}",
            frame.body.address,
            frame.body.nonce
        );
        let addr = frame.body.address.clone();
        let mut clients = context.clients.lock().await;
        clients.remove_client(&addr).await
    })
}


pub async fn send_offline(
    context: Arc<crate::context::Context>,
    client_type: &ClientType,
    data: Vec<u8>
) -> anyhow::Result<()> {
    let command = P2PCommand::new(Entity::Node as u8, Action::OffLine as u8, data);

    let frame = P2PFrame::build(context, command, 1).await.unwrap();
    // 2️⃣ 序列化 Frame
    let bytes = Codec::encode(&frame);
    send_bytes(&client_type, &bytes).await;
    // self.send(&bytes).await?;
    Ok(())
}
