use std::sync::Arc;

use aex::connection::context::Context;
use aex::tcp::types::Codec;
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::protocols::command::P2PCommand;
use crate::protocols::command::{Action, Entity};
use crate::protocols::frame::P2PFrame;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Encode, Decode)]
pub struct TickCommand {
    pub daily_epoch: u64,
    pub slot: u64,
    pub challenge: Vec<u8>,
}

impl Codec for TickCommand {}

pub async fn tick_handler(ctx: Arc<Mutex<Context>>, frame: P2PFrame, cmd: P2PCommand) {
    let tick: TickCommand = match Codec::decode(&cmd.data) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("❌ decode TickCommand failed: {e}");
            return;
        }
    };

    tracing::info!(
        "✅ Tick from {}: daily_epoch={}, slot={}",
        frame.body.address,
        tick.daily_epoch,
        tick.slot
    );

    let response = TickCommand {
        daily_epoch: tick.daily_epoch,
        slot: tick.slot,
        challenge: vec![],
    };

    let _ = P2PFrame::send::<TickCommand>(
        ctx.clone(),
        &Some(response),
        Entity::Node,
        Action::TickAck,
        false,
    )
    .await;
}

pub async fn send_tick(
    ctx: Arc<Mutex<Context>>,
    receiver: &str,
    daily_epoch: u64,
    slot: u64,
) -> anyhow::Result<()> {
    let tick = TickCommand {
        daily_epoch,
        slot,
        challenge: vec![],
    };

    P2PFrame::send::<TickCommand>(ctx.clone(), &Some(tick), Entity::Node, Action::Tick, false)
        .await?;

    tracing::info!(
        "Sent Tick to {}: daily_epoch={}, slot={}",
        receiver,
        daily_epoch,
        slot
    );
    Ok(())
}
