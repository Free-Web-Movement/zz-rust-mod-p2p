use std::sync::Arc;

use crate::{
    context::Context,
    nodes::servers::Servers,
    protocols::{
        command::{Command, Entity, NodeAction},
        defines::ClientType,
        frame::{Frame, FrameBody},
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
                println!(
                    "✅ Node Online: addr={}, nonce={}",
                    frame.body.address, frame.body.nonce
                );

                // 3️⃣ data 至少要有 flag
                if frame.body.data.len() < 1 {
                    eprintln!("❌ Online data too short");
                    return;
                }

                // 4️⃣ 拆 data
                let (endpoint_bytes, flag) = frame.body.data.split_at(frame.body.data.len() - 1);

                let is_inner = flag[0] == 0;

                // 5️⃣ 解 endpoint 列表
                let endpoints = match Servers::from_endpoints(endpoint_bytes.to_vec()) {
                    v => v,
                };

                // 6️⃣ 只处理 TCP client
                let tcp = match client_type {
                    ClientType::TCP(tcp) => tcp.clone(),
                    _ => {
                        eprintln!("❌ Online command not from TCP");
                        return;
                    }
                };

                // 7️⃣ 注册 client
                // for ep in endpoints {
                let addr = frame.body.address.clone();
                let mut clients = context.clients.lock().await;

                if is_inner {
                    clients.add_inner(&addr, tcp.clone());
                } else {
                    clients.add_external(&addr, tcp.clone());
                }
                // }
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
