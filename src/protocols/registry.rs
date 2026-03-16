use aex::{ on, tcp::router::Router };

use crate::protocols::{
    command::{ Action, Entity, P2PCommand },
    commands::{
        ack::onlineack_handler,
        message::message_handler,
        offline::offline_handler,
        online::online_handler,
    },
    frame::P2PFrame,
};
pub fn register(router: &mut Router) {
    // --- Node 模块 ---
    on!(router, P2PFrame, P2PCommand, [
        // 这里的 sync_limit_mid 会在执行 handler 前被 Router 线性调用
        [P2PCommand::to_u32(Entity::Node, Action::OnLine), online_handler],
        [P2PCommand::to_u32(Entity::Node, Action::OffLine), offline_handler],
        [P2PCommand::to_u32(Entity::Node, Action::OnLineAck), onlineack_handler],
        [P2PCommand::to_u32(Entity::Message, Action::SendText), message_handler],
    ]);
    println!("{:?}", router.handlers);
}
