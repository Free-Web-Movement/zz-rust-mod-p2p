use futures::future::BoxFuture;
use std::sync::Arc;

use crate::context::Context;
use crate::protocols::codec::CommandCodec;
use crate::protocols::frame::Frame;

/// 🔹 泛型命令处理器
pub struct CommandProcessor<C: Send + Sync + 'static, T: CommandCodec + Send + Sync + 'static> {
    pub entity: crate::protocols::command::Entity,
    pub action: crate::protocols::command::Action,

    /// 接收处理函数（on_receive）
    pub handler: fn(T, Frame, Arc<Context>, Arc<C>) -> BoxFuture<'static, ()>,

    /// 发送函数（send_to）
    pub sender: fn(T, Arc<Context>, Arc<C>) -> BoxFuture<'static, ()>,
}

impl<C: Send + Sync + 'static, T: CommandCodec + Send + Sync + 'static> CommandProcessor<C, T> {
    pub fn new(
        entity: crate::protocols::command::Entity,
        action: crate::protocols::command::Action,
        handler: fn(T, Frame, Arc<Context>, Arc<C>) -> BoxFuture<'static, ()>,
        sender: fn(T, Arc<Context>, Arc<C>) -> BoxFuture<'static, ()>,
    ) -> Self {
        Self {
            entity,
            action,
            handler,
            sender,
        }
    }

    /// 调用接收处理函数
    pub fn on(
        &self,
        cmd: T,
        frame: Frame,
        context: Arc<Context>,
        client: Arc<C>,
    ) -> BoxFuture<'static, ()> {
        (self.handler)(cmd, frame, context, client)
    }

    /// 调用发送函数
    pub fn to(&self, cmd: T, context: Arc<Context>, client: Arc<C>) -> BoxFuture<'static, ()> {
        (self.sender)(cmd, context, client)
    }
}
