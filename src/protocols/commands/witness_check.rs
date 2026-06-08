use std::sync::Arc;

use aex::connection::context::Context;
use aex::tcp::types::Codec;
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::node::Node;
use crate::protocols::command::P2PCommand;
use crate::protocols::command::{Action, Entity};
use crate::protocols::frame::P2PFrame;

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct StatusRequest {
    pub address: String,
    pub tick: u64,
    pub epoch: u64,
    pub timestamp: i64,
}

impl Codec for StatusRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct StatusAck {
    pub address: String,
    pub node_id: String,
    pub tick: u64,
    pub epoch: u64,
    pub timestamp: i64,
    pub is_online: bool,
    pub resources: Vec<u8>,
}

impl Codec for StatusAck {}

pub async fn status_request_handler(ctx: Arc<Mutex<Context>>, frame: P2PFrame, cmd: P2PCommand) {
    let req: StatusRequest = match Codec::decode(&cmd.data) {
        Ok(cmd) => cmd,
        Err(e) => {
            tracing::error!("❌ decode StatusRequest failed: {e}");
            return;
        }
    };

    tracing::info!(
        "✅ StatusRequest from {} at epoch={}, tick={}",
        req.address, req.epoch, req.tick
    );

    let node_id = {
        let guard = ctx.lock().await;
        guard
            .global
            .get::<Arc<Node>>()
            .await
            .map(|n| n.id.to_string())
            .unwrap_or_default()
    };

    let ack = StatusAck {
        address: frame.body.address.clone(),
        node_id,
        tick: req.tick,
        epoch: req.epoch,
        timestamp: chrono::Utc::now().timestamp(),
        is_online: true,
        resources: vec![],
    };

    if let Err(e) = P2PFrame::send::<StatusAck>(
        ctx.clone(),
        &Some(ack),
        Entity::Witness,
        Action::StatusAck,
        false,
    )
    .await
    {
        tracing::error!("❌ Failed to send StatusAck: {:?}", e);
    }
}

pub async fn status_ack_handler(_ctx: Arc<Mutex<Context>>, frame: P2PFrame, cmd: P2PCommand) {
    let ack: StatusAck = match Codec::decode(&cmd.data) {
        Ok(cmd) => cmd,
        Err(e) => {
            tracing::error!("❌ decode StatusAck failed: {e}");
            return;
        }
    };

    tracing::info!(
        "✅ StatusAck from {} (node {}): is_online={}",
        frame.body.address,
        ack.node_id,
        ack.is_online
    );
}
