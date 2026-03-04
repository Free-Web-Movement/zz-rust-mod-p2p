use std::sync::Arc;

use aex::connection::global::{self, GlobalContext};
use aex::tcp::types::Codec;
use bincode::{Decode, Encode};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWrite;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::Mutex;

use crate::context::Context;
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

pub async fn online_handler(
    global: Arc<GlobalContext>,
    frame: &mut P2PFrame,
    cmd: &mut P2PCommand,
    writer: &mut (dyn AsyncWrite + Send + Unpin),
) {
    let online: OnlineCommand = match Codec::decode(&cmd.data) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("❌ decode OnlineCommand failed: {e}");
            return;
        }
    };
}

pub fn on_online(
    cmd: P2PCommand,
    frame: P2PFrame,
    context: Arc<Context>,
    writer: Arc<Mutex<OwnedWriteHalf>>,
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

        let psk = context.paired_session_keys.clone();

        let guard = &mut *psk.lock().await;

        let ephemeral_public = match guard
            .establish_begins(
                frame.body.address.as_bytes().to_vec(),
                &online.ephemeral_public_key.to_vec(),
            )
            .await
        {
            Ok(k) => match k {
                Some(v) => v,
                None => {
                    return eprintln!(
                        "❌ Failed to establish session key: {:?}",
                        online.session_id
                    );
                }
            },
            Err(_) => {
                return eprintln!(
                    "❌ Failed to establish session key: {:?}",
                    online.session_id
                );
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
        // let (endpoints, is_inner) = match Servers::from_endpoints(online.endpoints) {
        //     (eps, flag) => (eps, flag == 0),
        // };
        // let addr = frame.body.address.clone();
        // let mut clients = context.clients.lock().await;
        // if is_inner {
        //     clients.add_inner(&addr, (*client_type).clone(), endpoints);
        // } else {
        //     clients.add_external(&addr, (*client_type).clone(), endpoints);
        // }

        // let writer = get_writer(&client_type).await;
        let mut guard = writer.lock().await;

        P2PFrame::send(
            &context.address,
            &mut *guard,
            &Some(ack),
            Entity::Node,
            Action::OnLineAck,
            None,
        )
        .await
        .expect("Error send online ack!");
    })
}
