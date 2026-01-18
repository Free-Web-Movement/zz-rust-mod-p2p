use futures::future::BoxFuture;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::context::Context;
use crate::protocols::command::{Action, Command, Entity};
use crate::protocols::frame::Frame;

/// Frame 处理器管理器，客户端类型泛型化
#[derive(Default)]
pub struct FrameHandlerRegistry<C = ()> {
    /// key = (Entity, Action)
    handlers: Mutex<
        HashMap<
            (Entity, Action),
            Arc<
                dyn Fn(Command, Frame, Arc<Context>, Arc<C>) -> BoxFuture<'static, ()>
                    + Send
                    + Sync,
            >,
        >,
    >,
}

impl<C: Send + Sync + 'static> FrameHandlerRegistry<C> {
    pub fn new() -> Self {
        Self {
            handlers: Mutex::new(HashMap::new()),
        }
    }

    /// 注册回调
    pub async fn register<F, Fut>(&self, entity: Entity, action: Action, handler: F)
    where
        F: Fn(Command, Frame, Arc<Context>, Arc<C>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut map = self.handlers.lock().await;
        map.insert(
            (entity, action),
            Arc::new(move |cmd, frame, ctx, client| Box::pin(handler(cmd, frame, ctx, client))),
        );
    }

    /// 调用处理
    pub async fn handle(&self, frame: Frame, context: Arc<Context>, client_type: Arc<C>) {
        let cmd = match frame.body.command_from_data() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("❌ Command decode failed: {:?}", e);
                return;
            }
        };

        let map = self.handlers.lock().await;
        if let Some(handler) = map.get(&(cmd.entity, cmd.action)) {
            handler(cmd, frame, context.clone(), client_type.clone()).await;
        } else {
            println!(
                "ℹ️ Unsupported command: entity={:?}, action={:?}",
                cmd.entity, cmd.action
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use crate::context::Context;
    use crate::protocols::command::{Action, Command, Entity};
    use crate::protocols::frame::Frame;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use zz_account::address::FreeWebMovementAddress;

    fn dummy_context() -> Context {
        let address = FreeWebMovementAddress::random();
        let storage = crate::nodes::storage::Storeage::new(None, None, None, None);
        let servers = crate::nodes::servers::Servers::new(
            address.clone(),
            storage,
            crate::nodes::net_info::NetInfo::new(8080),
        );
        Context::new("127.0.0.1".to_string(), 8080, address, servers)
    }

    // Mock ClientType，用于测试，不依赖真实 TCP
    #[derive(Clone)]
    struct MockClientType {
        pub sent: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl MockClientType {
        fn new() -> Self {
            Self {
                sent: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    // Mock send_bytes
    async fn mock_send_bytes(client: &MockClientType, data: &[u8]) {
        let mut sent = client.sent.lock().await;
        sent.push(data.to_vec());
    }

    #[tokio::test]
    async fn test_registry_with_mock_client() -> Result<()> {
        let context = Arc::new(dummy_context());
        let client = Arc::new(MockClientType::new());

        let registry = FrameHandlerRegistry::<MockClientType>::new();

        // 注册 Node OnLine 回调
        registry
            .register(Entity::Node, Action::OnLine, |cmd, frame, ctx, client| {
                Box::pin(async move {
                    println!("✅ Node OnLine triggered for {}", frame.body.address);

                    // 构造 OnLineAck 命令并发送给 mock client
                    let ack_cmd = Command::new(Entity::Node, Action::OnLineAck, Some(vec![42]));
                    let ack_frame = Frame::build_frame(ctx.clone(), ack_cmd, 1).await.unwrap();
                    mock_send_bytes(&client, &Frame::to(ack_frame)).await;
                })
            })
            .await;

        // 构造测试 Frame
        let cmd = Command::new(Entity::Node, Action::OnLine, Some(vec![1, 2, 3]));
        let frame = Frame::build_frame(context.clone(), cmd, 1).await?;

        // 调用处理
        registry
            .handle(frame, context.clone(), client.clone())
            .await;

        // 验证 mock client 收到数据
        let sent = client.sent.lock().await;
        assert_eq!(sent.len(), 1, "应该发送了一条数据");

        Ok(())
    }
}
