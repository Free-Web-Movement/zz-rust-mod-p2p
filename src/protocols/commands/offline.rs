use std::sync::Arc;

use futures::future::BoxFuture;

use crate::context::Context;
use crate::protocols::client_type::{ ClientType, send_bytes };
use crate::protocols::codec::Codec;
use crate::protocols::command::{ Action, Command, Entity };
use crate::protocols::frame::Frame;

pub fn on_offline(
    cmd: Command,
    frame: Frame,
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
    data: Option<Vec<u8>>
) -> anyhow::Result<()> {
    let command = Command::new(Entity::Node as u8, Action::OffLine as u8, data);

    let frame = Frame::build(context, command, 1).await.unwrap();
    // 2️⃣ 序列化 Frame
    let bytes = Frame::to_bytes(&frame);
    send_bytes(&client_type, &bytes).await;
    // self.send(&bytes).await?;
    Ok(())
}
