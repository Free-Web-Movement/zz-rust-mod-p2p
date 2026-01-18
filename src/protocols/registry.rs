use std::collections::HashMap;
use std::sync::Arc;

use futures::future::BoxFuture;
use tokio::sync::Mutex;

use crate::{
    context::Context,
    protocols::{
        client_type::ClientType,
        command::{ Action, Entity },
        commands::{ ack::ack_processor, online::online_processor },
        frame::Frame,
        processor::CommandProcessor,
    },
};

/// 🔹 命令处理注册中心
#[derive(Default)]
pub struct FrameHandlerRegistry<C: Send + Sync + 'static> {
    handlers: Mutex<
        HashMap<
            (Entity, Action),
            Arc<dyn (Fn(Frame, Arc<Context>, Arc<C>) -> BoxFuture<'static, ()>) + Send + Sync>
        >
    >,
}

impl<C: Send + Sync + 'static> FrameHandlerRegistry<C> {
    /// 构造
    pub fn new() -> Self {
        Self {
            handlers: Mutex::new(HashMap::new()),
        }
    }

    /// 注册单个命令
    pub async fn register(&self, cmd: &CommandProcessor<C>) {
        let mut map = self.handlers.lock().await;
        map.insert((cmd.entity, cmd.action), cmd.handler.clone());
    }

    /// 批量注册命令
    pub async fn register_all(&self, cmds: &[CommandProcessor<C>]) {
        for cmd in cmds {
            self.register(cmd).await;
        }
    }

    /// 处理 Frame
    pub async fn handle(&self, frame: Frame, ctx: Arc<Context>, client: Arc<C>) {
        println!("inside registry handling!");
        let cmd = frame.body.command_from_data().unwrap();
        let entity = cmd.entity;
        let action = cmd.action;

        println!("inside command {:?}!", cmd);

        let map = self.handlers.lock().await;
        if let Some(handler) = map.get(&(entity, action)) {
            // 调用 handler
            println!("inside hanlder!");
            handler(frame, ctx, client).await;
        } else {
            tracing::info!("⚠️ Unsupported command: entity={:?}, action={:?}", entity, action);
        }
    }

    pub async fn register_processor(processor: &[CommandProcessor<ClientType>]) {
        frame_handler_registry.register_all(processor).await;
    }

    pub async fn init_registry() {
        let processors = [online_processor(), ack_processor()];
        Self::register_processor(&processors).await;
    }
}

// 假设这里有全局 registry
lazy_static::lazy_static! {
    pub static ref frame_handler_registry: FrameHandlerRegistry<ClientType> =
        FrameHandlerRegistry::new();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use crate::protocols::codec::Codec;
    use crate::protocols::command::{ Action, Command, Entity };
    use crate::protocols::frame::Frame;
    use anyhow::Result;
    use bincode::{ Decode, Encode };
    use serde::{ Deserialize, Serialize };
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use zz_account::address::FreeWebMovementAddress;

    fn dummy_context() -> Context {
        let address = FreeWebMovementAddress::random();
        let storage = crate::nodes::storage::Storeage::new(None, None, None, None);
        let servers = crate::nodes::servers::Servers::new(
            address.clone(),
            storage,
            crate::nodes::net_info::NetInfo::new(8080)
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
    #[derive(Clone, Encode, Decode, Serialize, Deserialize)]
    struct TempCommand {
        pub id: u8,
    }

    impl Codec for TempCommand {}

    #[tokio::test]
    async fn test_registry_with_mock_client() -> Result<()> {
        let context = Arc::new(dummy_context());
        let client = Arc::new(MockClientType::new());

        // 构造命令处理器
        let processor = CommandProcessor::new(
            Entity::Node,
            Action::OnLine,
            |
                cmd: Command,
                _frame: Frame,
                ctx: Arc<crate::context::Context>,
                client: Arc<MockClientType>
            | {
                Box::pin(async move {
                    // 简单记录收到命令
                    println!("✅ Node OnLine triggered with id={:?}", cmd.action);

                    // 发送 ack
                    let ack: Command = Command::new(Entity::Node, Action::OnLineAck, Some(vec![0]));
                    let ack_frame = Frame::build(ctx.clone(), ack, 1).await.unwrap();
                    mock_send_bytes(&client, &Frame::to_bytes(&ack_frame)).await;
                })
            },
            |cmd: TempCommand, _ctx: Arc<crate::context::Context>, client: Arc<MockClientType>| {
                Box::pin(async move {
                    println!("📤 Sending command id={}", cmd.id);
                    let data = cmd.to_bytes();
                    mock_send_bytes(&client, &data).await;
                })
            }
        );

        let registry = FrameHandlerRegistry::<MockClientType>::new();

        // 注册 Node OnLine 回调
        registry.register(&processor).await;

        // 构造测试 Frame
        let cmd = Command::new(Entity::Node, Action::OnLine, Some(vec![1, 2, 3]));
        let frame = Frame::build(context.clone(), cmd, 1).await?;

        // 调用处理
        registry.handle(frame, context.clone(), client.clone()).await;

        // 验证 mock client 收到数据
        let sent = client.sent.lock().await;
        assert_eq!(sent.len(), 1, "应该发送了一条数据");

        Ok(())
    }

    #[tokio::test]
    async fn test_registry_with_multiple_commands() -> anyhow::Result<()> {
        use bincode::{ Encode, Decode };
        use serde::{ Serialize, Deserialize };

        let context = Arc::new(dummy_context());
        let client = Arc::new(MockClientType::new());

        // 定义两个命令类型
        #[derive(Clone, Encode, Decode, Serialize, Deserialize)]
        struct CmdA {
            pub value: u8,
        }
        #[derive(Clone, Encode, Decode, Serialize, Deserialize)]
        struct CmdB {
            pub value: u16,
        }

        impl Codec for CmdA {}
        impl Codec for CmdB {}

        // 构造 CommandProcessor
        let processor_a = CommandProcessor::new(
            Entity::Node,
            Action::OnLine,
            |cmd: Command, frame: Frame, ctx: Arc<Context>, client: Arc<MockClientType>| {
                Box::pin(async move {
                    println!("✅ CmdA triggered: {:?}", cmd.entity);
                    let ack = Command::new(Entity::Node, Action::OnLineAck, Some(vec![]));
                    let ack_frame = Frame::build(ctx.clone(), ack, 1).await.unwrap();
                    mock_send_bytes(&client, &Frame::to_bytes(&ack_frame)).await;
                })
            },
            |cmd: CmdA, ctx: Arc<Context>, client: Arc<MockClientType>| {
                Box::pin(async move {
                    println!("📤 Sending CmdA: {}", cmd.value);
                    mock_send_bytes(&client, &cmd.to_bytes()).await;
                })
            }
        );

        let processor_b = CommandProcessor::new(
            Entity::Node,
            Action::OffLine,
            |cmd: Command, frame: Frame, ctx: Arc<Context>, client: Arc<MockClientType>| {
                Box::pin(async move {
                    println!("✅ CmdB triggered: {:?}", cmd.entity);
                    let ack = Command::new(Entity::Node, Action::OnLineAck, Some(vec![0]));
                    let ack_frame = Frame::build(ctx.clone(), ack, 2).await.unwrap();
                    mock_send_bytes(&client, &Frame::to_bytes(&ack_frame)).await;
                })
            },
            |cmd: CmdB, ctx: Arc<Context>, client: Arc<MockClientType>| {
                Box::pin(async move {
                    println!("📤 Sending CmdB: {}", cmd.value);
                    mock_send_bytes(&client, &cmd.to_bytes()).await;
                })
            }
        );

        let registry = FrameHandlerRegistry::<MockClientType>::new();

        // 批量注册
        registry.register_all(&[processor_a, processor_b]).await;

        // 构造 Frame 并触发 CmdA
        let frame_a = Frame::build(
            context.clone(),
            Command::new(Entity::Node, Action::OnLine, Some(vec![7])),
            1
        ).await?;
        registry.handle(frame_a, context.clone(), client.clone()).await;

        // 构造 Frame 并触发 CmdB
        let frame_b = Frame::build(
            context.clone(),
            Command::new(Entity::Node, Action::OffLine, Some(vec![0x12, 0x34])),
            2
        ).await?;
        registry.handle(frame_b, context.clone(), client.clone()).await;

        // 验证 mock client 收到两条数据
        let sent = client.sent.lock().await;
        assert_eq!(sent.len(), 2, "应该发送了两条数据");

        Ok(())
    }
}
