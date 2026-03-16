use std::sync::Arc;

use aex::connection::context::Context;
use aex::connection::node::Node;
use aex::tcp::types::Codec;
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use zz_account::address::FreeWebMovementAddress;

use crate::protocols::command::P2PCommand;
use crate::protocols::command::{Action, Entity};
use crate::protocols::commands::ack::OnlineAckCommand;
use crate::protocols::frame::P2PFrame;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Encode, Decode)]
pub struct OnlineCommand {
    pub session_id: Vec<u8>, // 临时 session id
    pub node: Node,
    pub ephemeral_public_key: [u8; 32],
}

// ⚡ 实现 CommandCodec，移除 to_bytes/from_bytes
impl Codec for OnlineCommand {}

pub async fn online_handler(
    ctx: Arc<Mutex<Context>>,
    frame: P2PFrame,
    cmd: P2PCommand, // writer: &mut (dyn AsyncWrite + Send + Unpin),
) {
    println!("inside online handler!");
    let online: OnlineCommand = match Codec::decode(&cmd.data) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("❌ decode OnlineCommand failed: {e}");
            return ();
        }
    };
    println!(
        "✅ Node Online: addr={}, nonce={}",
        frame.body.address, frame.body.nonce
    );

    // ===== 1️⃣ OnlineCommand 解码 =====

    println!("received session_id: {:?}", online.session_id);

    let psk = {
        let ctx = ctx.lock().await;
        ctx.global.paired_session_keys.clone().unwrap()
    };

    // 在这个作用域里，psk 是 &PairedSessionKey
    // 把它传给需要它的函数

    let ephemeral_public = {
        let guard = psk.lock().await;
        guard
            .establish_begins(
                frame.body.address.as_bytes().to_vec(),
                &online.ephemeral_public_key.to_vec(),
            )
            .await
            .unwrap()
            .unwrap()
    };

    let address: FreeWebMovementAddress = {
        let ctx = ctx.lock().await;
        ctx.get().await.expect("Expect Address be set!")
    };

    let local_node = {
        let ctx = ctx.lock().await;
        ctx.global.local_node.clone()
    };

    let node = {
        let guard = local_node.write().await;
        guard.clone()
    };

    let ack = OnlineAckCommand {
        session_id: online.session_id,
        address: address.to_string(),
        node,
        ephemeral_public_key: ephemeral_public.to_bytes(),
    };

    println!("send ack session_id : {:?}", ack.session_id);
    println!("send ack: {:?}", Codec::encode(&ack));

    // 4. 执行异步发送
    P2PFrame::send::<OnlineAckCommand>(
        ctx.clone(),
        &Some(ack),
        Entity::Node,
        Action::OnLineAck,
        false,
    )
    .await
    .expect("Error send online ack!");

    {
        let cloned = ctx.clone();
        let guard = ctx.lock().await;
        guard.global.manager.update(guard.addr, true, Some(cloned));
    }
}
