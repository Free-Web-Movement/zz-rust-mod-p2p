use std::sync::Arc;

use aex::tcp::types::Codec;
use bincode::{Decode, Encode};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};

use crate::context::Context;
use crate::nodes::servers::Servers;
use crate::protocols::client_type::{ClientType, get_writer};
use crate::protocols::command::P2PCommand;
use crate::protocols::command::{Action, Entity};
use crate::protocols::commands::ack::OnlineAckCommand;
use crate::protocols::frame::P2PFrame;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Encode, Decode)]
pub struct OnlineCommand {
    pub session_id: Vec<u8>, // 临时 session id
    pub endpoints: Vec<u8>,
    pub ephemeral_public_key: [u8; 32],
}

// ⚡ 实现 CommandCodec，移除 to_bytes/from_bytes
impl Codec for OnlineCommand {}

pub fn on_online(
    cmd: P2PCommand,
    frame: P2PFrame,
    context: Arc<Context>,
    client_type: Arc<ClientType>,
) -> BoxFuture<'static, ()> {
    Box::pin(async move {
        println!(
            "✅ Node Online: addr={}, nonce={}",
            frame.body.address, frame.body.nonce
        );

        // ===== 1️⃣ OnlineCommand 解码 =====

        let online: OnlineCommand = match Codec::decode(&cmd.data) {
            Ok(cmd) => cmd,
            Err(e) => {
                eprintln!("❌ decode OnlineCommand failed: {e}");
                return;
            }
        };

        println!("received session_id: {:?}", online.session_id);

        let ephemeral_public = match context
            .paired_session_keys
            .establish_begins(
                frame.body.address.as_bytes().to_vec(),
                &online.ephemeral_public_key.to_vec(),
            )
            .await
        {
            Ok(k) => match k {
                Some(v) => v,
                None => {
                    return eprintln!("❌ Failed to establish session key: {:?}", online.session_id);
                }
            },
            Err(_) => {
                return eprintln!("❌ Failed to establish session key: {:?}", online.session_id);
            }
        };

        // ===== 3️⃣ 构造 OnlineAckCommand =====
        let ack = OnlineAckCommand {
            session_id: online.session_id,
            address: context.address.to_string(),
            ephemeral_public_key: ephemeral_public.to_bytes(),
        };

        println!("send ack session_id : {:?}", ack.session_id);
        println!("send ack: {:?}", Codec::encode(&ack));

        // ===== 4️⃣ clients 登记 =====
        let (endpoints, is_inner) = match Servers::from_endpoints(online.endpoints) {
            (eps, flag) => (eps, flag == 0),
        };
        let addr = frame.body.address.clone();
        let mut clients = context.clients.lock().await;
        if is_inner {
            clients.add_inner(&addr, (*client_type).clone(), endpoints);
        } else {
            clients.add_external(&addr, (*client_type).clone(), endpoints);
        }

        let writer = get_writer(&client_type).await;
        let mut guard = writer.lock().await;


        P2PFrame::send(&context.address, &mut *guard, &Some(ack), 
        Entity::Node as u8, Action::OnLineAck as u8).await.expect("Error send online ack!");
    })
}
