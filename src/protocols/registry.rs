// use std::collections::HashMap;
// use std::sync::Arc;

// use tokio::{net::tcp::OwnedWriteHalf, sync::Mutex};

// use crate::{
//     context::Context,
//     protocols::{
//         command::{ Action, CommandCallback, Entity },
//         commands::{
//             ack::on_online_ack,
//             message::on_text_message,
//             offline::on_offline,
//             online::on_online,
//         },
//         frame::P2PFrame,
//     },
// };
// /// 🔹 命令处理注册中心
// #[derive(Default)]
// pub struct CommandHandlerRegistry {
//     handlers: Mutex<HashMap<(u8, u8), Arc<CommandCallback>>>,
// }

// impl CommandHandlerRegistry {
//     /// 构造
//     pub fn new() -> Self {
//         Self {
//             handlers: Mutex::new(HashMap::new()),
//         }
//     }

//     /// 处理 Frame
//     pub async fn handle(&self, frame: P2PFrame, ctx: Arc<Context>, writer: Arc<Mutex<OwnedWriteHalf>>) {
//         println!("inside registry handling!");
//         let cmd = frame.body.command_from_data().unwrap();
//         let entity = cmd.entity;
//         let action = cmd.action;

//         println!("inside command {:?}!", cmd);

//         let map = self.handlers.lock().await;
//         if let Some(handler) = map.get(&(entity, action)) {
//             // 调用 handler
//             println!("inside hanlder!");
//             handler(cmd, frame, ctx, writer).await;
//         } else {
//             tracing::info!("⚠️ Unsupported command: entity={:?}, action={:?}", entity, action);
//         }
//     }
//     pub async fn init_registry() {
//         let processors = vec![
//             (Entity::Node as u8, Action::OnLine as u8, on_online as CommandCallback),
//             (Entity::Node as u8, Action::OnLineAck as u8, on_online_ack as CommandCallback),
//             (Entity::Node as u8, Action::OffLine as u8, on_offline as CommandCallback),
//             (Entity::Message as u8, Action::SendText as u8, on_text_message as CommandCallback)
//         ];

//         let mut map = command_handler_registry.handlers.lock().await;

//         for processor in processors.iter() {
//             map.insert((processor.0, processor.1), Arc::new(processor.2));
//         }
//     }

//     pub async fn register(&self, entity: u8, action: u8, handler: CommandCallback) {
//         let mut map = self.handlers.lock().await;
//         map.insert((entity, action), Arc::new(handler));
//     }

//     pub async fn on(frame: P2PFrame, ctx: Arc<Context>, writer: Arc<Mutex<OwnedWriteHalf>>) {
//         command_handler_registry.handle(frame, ctx, writer).await;
//     }
//     pub async fn clear(&self) {
//         let mut map = self.handlers.lock().await;
//         map.clear();
//     }
//     pub async fn unregister(&self, entity: u8, action: u8) {
//         let mut map = self.handlers.lock().await;
//         map.remove(&(entity, action));
//     }

//     pub fn get_instance() -> &'static CommandHandlerRegistry {
//         &command_handler_registry
//     }
// }

// // 假设这里有全局 registry
// lazy_static::lazy_static! {
//     static ref command_handler_registry: CommandHandlerRegistry = CommandHandlerRegistry::new();
// }

use std::sync::Arc;

use aex::{connection::context::Context, tcp::router::Router};

use crate::protocols::{
    command::{Action, Entity, P2PCommand},
    commands::online::online_handler,
    frame::P2PFrame,
};
use tokio::sync::Mutex;

pub fn register(router: &mut Router) {
    router.on::<P2PFrame, P2PCommand, _, _>(
        P2PCommand::to_u32(Entity::Node, Action::OnLine),
      |ctx_arc: Arc<Mutex<Context<'_>>>, f, c| {
    // 1. 在同步层（异步块外）把所有需要的数据“解构”出来
    // 这一步彻底切断了数据与 '1 生命周期的联系
    let (mut r_owned, mut w_owned, global, addr, local) = {
        let mut guard = ctx_arc.blocking_lock();
        (
            guard.reader.take(), // 提取 Box
            guard.writer.take(), 
            guard.global.clone(),
            guard.addr,
            guard.local.clone()
        )
    };

    // 2. 将引用类型转为所有权类型
    let mut f_owned = f.clone();
    let mut c_owned = c.clone();

    // ⚡ 这里的关键：我们将 ctx_arc 留在外面，不 move 进去
    // 或者即使 move 进去，也不要在异步块里访问它
    async move {
        // 3. 在异步块内部重建临时 Context
        // 这个 temp_ctx 借用的是 async 块拥有的 r_owned，所以是安全的
        let mut temp_ctx = Context::new(
            &mut r_owned,
            &mut w_owned,
            global,
            addr,
        );
        temp_ctx.local = local;

        // 4. 执行业务
        let result = online_handler(&mut temp_ctx, &mut f_owned, &mut c_owned).await;

        // 5. ⚡ 如果你必须归还 IO，不能直接用之前的 ctx_arc
        // 你需要把结果和 IO 返回给外层，由外层负责塞回去
        // 或者使用下面提到的“无生命周期 Context”重构
        
        result;
        Ok(true)
    }
}
    );
}
