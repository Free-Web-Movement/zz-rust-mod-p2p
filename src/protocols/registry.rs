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

use aex::tcp::router::Router;

use crate::protocols::{
    command::{Action, Entity, P2PCommand},
    commands::online::online_handler,
    frame::P2PFrame,
};

pub fn register(router: &mut Router) {
    router.on::<P2PFrame, P2PCommand, _, _>(
        P2PCommand::to_u32(Entity::Node, Action::OnLine),
        |g, f, c, _r, mut w| {
            let g = g.clone();
            let mut f = f.clone();
            let mut c = c.clone();
            async move {
                let _ = online_handler(g, &mut f, &mut c, w.as_mut()).await;
                Ok(false)
            }
        },
    );
}
