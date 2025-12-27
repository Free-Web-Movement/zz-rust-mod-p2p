use std::sync::Arc;

use crate::{
    context::Context,
    protocols::{
        command::{Entity, NodeAction},
        defines::ClientType,
        frame::Frame,
    },
};

#[derive(Clone)]
pub struct CommandParser;
impl CommandParser {
    pub async fn on(frame: &Frame, context: Arc<Context>, client_type: &ClientType) {
        // 1️⃣ 解 Command
        let cmd = match frame.body.command_from_data() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("❌ Command decode failed: {:?}", e);
                return;
            }
        };

        // 2️⃣ 只处理 Node Online / Offline
        match (cmd.entity as Entity, cmd.action as NodeAction) {
            (Entity::Node, NodeAction::OnLine) => {
              Self::on_node_online(frame, context, client_type).await;
            }

            (Entity::Node, NodeAction::OffLine) => {
                println!(
                    "⚠️ Node Offline: addr={}, nonce={}",
                    frame.body.address, frame.body.nonce
                );
                // 这里你以后可以做 remove
            }

            _ => {
                println!(
                    "ℹ️ Unsupported command: entity={:?}, action={:?}",
                    cmd.entity, cmd.action
                );
            }
        }
    }
}
