use futures::future::BoxFuture;
use std::sync::Arc;

use crate::context::Context;
use crate::protocols::command::{Action, Command, Entity};
use crate::protocols::frame::Frame;

/// 命令注册描述，支持接收和发送
pub struct CommandProcessor<C: Send + Sync + 'static> {
    pub entity: Entity,
    pub action: Action,

    /// 接收处理函数（on_receive）
    pub handler: fn(Command, Frame, Arc<Context>, Arc<C>) -> BoxFuture<'static, ()>,

    /// 发送函数（send_to）
    pub sender: fn(Vec<u8>, Arc<Context>, Arc<C>) -> BoxFuture<'static, ()>,
}

impl<C: Send + Sync + 'static> CommandProcessor<C> {
    pub fn new(
        entity: Entity,
        action: Action,
        handler: fn(Command, Frame, Arc<Context>, Arc<C>) -> BoxFuture<'static, ()>,
        sender: fn(Vec<u8>, Arc<Context>, Arc<C>) -> BoxFuture<'static, ()>,
    ) -> Self {
        Self {
            entity,
            action,
            handler,
            sender,
        }
    }
}
