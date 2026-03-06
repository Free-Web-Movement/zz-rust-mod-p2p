use std::sync::Arc;

use aex::connection::context::Context;
use aex::tcp::types::Codec;
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

// use crate::context::Context;
use crate::protocols::command::P2PCommand;
use crate::protocols::frame::P2PFrame;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Encode, Decode)]
pub struct OfflineCommand {
    pub session_id: Vec<u8>, // 临时 session id
    pub endpoints: Vec<u8>,
}

// ⚡ 实现 CommandCodec，移除 to_bytes/from_bytes
impl Codec for OfflineCommand {}

pub async fn onffline_handler(
    ctx: Arc<Mutex<Context>>,
    frame: P2PFrame,
    _cmd: P2PCommand,
) {
    // 处理 Node Offline 命令的逻辑
    println!(
        "Node Offline Command Received: addr={}, nonce={}",
        frame.body.address, frame.body.nonce
    );

    // should remove address info for future handler

    let guard = ctx.lock().await;
    guard.global.manager.remove(guard.addr, true);
}
