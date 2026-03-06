use std::sync::Arc;

use aex::connection::context::Context;
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
    pub endpoints: Vec<u8>,
    pub ephemeral_public_key: [u8; 32],
}

// ⚡ 实现 CommandCodec，移除 to_bytes/from_bytes
impl Codec for OnlineCommand {}

pub async fn online_handler(
    ctx: Arc<Mutex<Context>>,
    frame: P2PFrame,
    cmd: P2PCommand,
    // writer: &mut (dyn AsyncWrite + Send + Unpin),
) {
    let online: OnlineCommand = match Codec::decode(&cmd.data) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("❌ decode OnlineCommand failed: {e}");
            return ()
        }
    };
    println!(
        "✅ Node Online: addr={}, nonce={}",
        frame.body.address, frame.body.nonce
    );

    // ===== 1️⃣ OnlineCommand 解码 =====

    println!("received session_id: {:?}", online.session_id);
    let mut ctx = ctx.lock().await;

    if let Some(ref psk) = ctx.global.paired_session_keys {
        // 在这个作用域里，psk 是 &PairedSessionKey
        // 把它传给需要它的函数
        let ephemeral_public = match psk
            .establish_begins(
                frame.body.address.as_bytes().to_vec(),
                &online.ephemeral_public_key.to_vec(),
            )
            .await
        {
            Ok(k) => match k {
                Some(v) => v,
                None => {
                 eprintln!(
                        "❌ Failed to establish session key: {:?}",
                        online.session_id
                    );
                    return  ();
                }
            },
            Err(_) => {
             eprintln!(
                    "❌ Failed to establish session key: {:?}",
                    online.session_id
                );
                return ();
            }
        };
        let address: FreeWebMovementAddress = ctx.get().await.expect("Expect Address be set!");
        let ack = OnlineAckCommand {
            session_id: online.session_id,
            address: address.to_string(),
            ephemeral_public_key: ephemeral_public.to_bytes(),
        };

        println!("send ack session_id : {:?}", ack.session_id);
        println!("send ack: {:?}", Codec::encode(&ack));
        // let writer = ctx.writer;
        // ctx.global.manager.add(ctx.addr, writer, handle, true);
        // 假设 ctx.writer 是 &mut Option<Box<dyn AsyncWrite...>>
        if let Some(w) = ctx.writer.as_deref_mut() {
            P2PFrame::send::<OnlineAckCommand>(
                &address,
                w, // 这里的 w 已经是 &mut dyn AsyncWrite 了
                &Some(ack),
                Entity::Node,
                Action::OnLineAck,
                None,
            )
            .await
            .expect("Error send online ack!");
        }
        match ctx.writer.take() {
            Some(writer) => {
                ctx.global
                    .manager
                    .update(ctx.addr, true, Arc::new(Mutex::new(writer)));
                return ();
            }
            None => {
                return ();
            },
        }
    }  else {
        // 处理没有密钥的情况
        return ();
    }

}