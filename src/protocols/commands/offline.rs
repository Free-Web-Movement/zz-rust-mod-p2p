use std::sync::Arc;

use aex::tcp::types::Codec;
use bincode::{Decode, Encode};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};

use crate::context::Context;
use crate::protocols::client_type::{ ClientType };
use crate::protocols::command::{ P2PCommand };
use crate::protocols::frame::P2PFrame;


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Encode, Decode)]
pub struct OfflineCommand {
    pub session_id: Vec<u8>, // 临时 session id
    pub endpoints: Vec<u8>,
}

// ⚡ 实现 CommandCodec，移除 to_bytes/from_bytes
impl Codec for OfflineCommand {}


pub fn on_offline(
    _: P2PCommand,
    frame: P2PFrame,
    context: Arc<Context>,
    __: Arc<ClientType>
) -> BoxFuture<'static, ()> {
    Box::pin(async move {
        // 处理 Node Offline 命令的逻辑
        println!(
            "Node Offline Command Received: addr={}, nonce={}",
            frame.body.address,
            frame.body.nonce
        );
        let addr = frame.body.address.clone();
        let mut clients = context.clients.lock().await;
        clients.remove_client(&addr).await
    })
}