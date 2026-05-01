use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

use aex::connection::context::Context;
use aex::connection::node::Node;
use aex::tcp::types::Codec;
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use crate::protocols::command::P2PCommand;
use crate::protocols::command::{Action, Entity};
use crate::protocols::commands::ack::{SeedRecord, SeedsCommand};
use crate::protocols::commands::online::get_all_ips;
use crate::protocols::commands::online::OnlineCommand;
use crate::protocols::frame::P2PFrame;

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct TickCommand {
    pub daily_epoch: u64,
    pub slot: u64,
    pub challenge: Vec<u8>,
    pub pre_hash: [u8; 32],
    pub seeds: Option<SeedsCommand>,
}

impl Codec for TickCommand {}

pub async fn init_witness_spread(ctx: Arc<Mutex<Context>>) {
    let ctx_clone = ctx.clone();
    let global = {
        let guard = ctx_clone.lock().await;
        guard.global.clone()
    };
    
    let _ = global.spread.subscribe::<TickCommand>("witness_tick", Box::new(move |tick| {
        let ctx2 = ctx_clone.clone();
        Box::pin(async move {
            handle_witness_tick(ctx2, tick).await;
        })
    })).await;
}

async fn handle_witness_tick(ctx: Arc<Mutex<Context>>, tick: TickCommand) {
    let seeds_cmd = match tick.seeds {
        Some(ref cmd) if cmd.verify() => cmd,
        _ => return,
    };

    let target_addrs: Vec<String> = seeds_cmd.seeds.iter().map(|s| s.address.clone()).collect();
    
    let current_entries: Vec<String> = {
        let guard = ctx.lock().await;
        guard.global.manager.get_all_entries()
            .iter()
            .map(|a| a.to_string())
            .collect()
    };

    for addr_str in target_addrs {
        if !current_entries.contains(&addr_str) {
            if let Ok(addr) = addr_str.parse::<SocketAddr>() {
                let ctx_clone = ctx.clone();
                let addr_owned = addr_str;
                tokio::spawn(async move {
                    if let Err(e) = connect_to_peer(ctx_clone, addr).await {
                        eprintln!("  ❌ Spread connect failed: {}: {:?}", addr_owned, e);
                    }
                });
            }
        }
    }
}

pub async fn broadcast_witness_tick(ctx: Arc<Mutex<Context>>, daily_epoch: u64, slot: u64) {
    let tick = build_tick_command(ctx.clone(), daily_epoch, slot).await;

    let global = {
        let guard = ctx.lock().await;
        guard.global.clone()
    };
    global.spread.publish("witness_tick", tick).await.ok();
}

async fn build_tick_command(ctx: Arc<Mutex<Context>>, daily_epoch: u64, slot: u64) -> TickCommand {
    let entries = {
        let guard = ctx.lock().await;
        guard.global.manager.get_all_entries()
    };
    
    let seed_records: Vec<SeedRecord> = entries
        .iter()
        .map(|addr| SeedRecord::new(addr.to_string()))
        .collect();

    let pre_hash = if seed_records.is_empty() {
        [0u8; 32]
    } else {
        let mut hasher = Sha256::new();
        for addr in &entries {
            hasher.update(addr.to_string().as_bytes());
        }
        hasher.finalize().into()
    };

    TickCommand {
        daily_epoch,
        slot,
        challenge: vec![],
        pre_hash,
        seeds: if seed_records.is_empty() {
            None
        } else {
            Some(SeedsCommand::new(seed_records))
        },
    }
}

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

    broadcast_witness_tick(ctx.clone(), tick.daily_epoch, tick.slot).await;

    let new_comers = if let Some(ref seeds_cmd) = tick.seeds {
        if seeds_cmd.verify() {
            sync_witness_ring(ctx.clone(), seeds_cmd, tick.pre_hash).await
        } else {
            eprintln!("❌ Invalid seeds hash in tick!");
            vec![]
        }
    } else {
        vec![]
    };

    let (response_seeds, new_hash) = build_witness_hash(ctx.clone(), &new_comers, tick.pre_hash).await;

    let response = TickCommand {
        daily_epoch: tick.daily_epoch,
        slot: tick.slot,
        challenge: vec![],
        pre_hash: new_hash,
        seeds: response_seeds,
    };

    let _ = P2PFrame::send::<TickCommand>(
        ctx.clone(),
        &Some(response),
        Entity::Node,
        Action::TickAck,
        false,
    ).await;
}

async fn sync_witness_ring(
    ctx: Arc<Mutex<Context>>,
    seeds_cmd: &SeedsCommand,
    _pre_hash: [u8; 32],
) -> Vec<SeedRecord> {
    let target_addrs: Vec<String> = seeds_cmd.seeds.iter().map(|s| s.address.clone()).collect();
    let mut new_comers = Vec::new();
    
    {
        let guard = ctx.lock().await;
        let manager = guard.global.manager.clone();
        let current_entries: HashSet<String> = manager.get_all_entries()
            .iter()
            .map(|a| a.to_string())
            .collect();

        for addr_str in &target_addrs {
            if !current_entries.contains(addr_str) {
                if let Ok(addr) = addr_str.parse::<SocketAddr>() {
                    new_comers.push((addr, addr_str.clone()));
                }
            }
        }
    }

    if new_comers.is_empty() {
        return vec![];
    }

    println!("🔄 Sync: {} new peers to connect", new_comers.len());

    for (addr, addr_str) in new_comers {
        let ctx_clone = ctx.clone();
        let addr_copy = addr;
        let addr_str_owned = addr_str;
        tokio::spawn(async move {
            if let Err(e) = connect_to_peer(ctx_clone, addr_copy).await {
                eprintln!("  ❌ Sync connect failed: {}: {:?}", addr_str_owned, e);
            }
        });
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let connected: Vec<SeedRecord> = {
        let guard = ctx.lock().await;
        let manager = guard.global.manager.clone();
        manager.get_all_entries()
            .iter()
            .map(|addr| SeedRecord::new(addr.to_string()))
            .collect()
    };

    connected
}

async fn build_witness_hash(
    ctx: Arc<Mutex<Context>>,
    _new_comers: &[SeedRecord],
    pre_hash: [u8; 32],
) -> (Option<SeedsCommand>, [u8; 32]) {
    let (entries, new_hash) = {
        let guard = ctx.lock().await;
        let manager = guard.global.manager.clone();
        let entries = manager.get_all_entries();
        
        let mut hasher = Sha256::new();
        hasher.update(&pre_hash);
        for addr in &entries {
            hasher.update(addr.to_string().as_bytes());
        }
        let new_hash: [u8; 32] = hasher.finalize().into();
        
        (entries, new_hash)
    };

    let seed_records: Vec<SeedRecord> = entries
        .iter()
        .map(|addr| SeedRecord::new(addr.to_string()))
        .collect();

    let seeds_cmd = if seed_records.is_empty() {
        None
    } else {
        Some(SeedsCommand::new(seed_records))
    };

    (seeds_cmd, new_hash)
}

async fn connect_to_peer(ctx: Arc<Mutex<Context>>, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let psk = {
        let guard = ctx.lock().await;
        guard.global.paired_session_keys.clone().unwrap()
    };
    
    let (id, key) = {
        let guard = psk.lock().await;
        guard.create(false).await
    };
    
    let aex_node = Node::from_system(addr.port(), id.clone(), 1);
    let announced_ips = get_all_ips();
    
    let cmd = OnlineCommand {
        session_id: id,
        node: aex_node,
        ephemeral_public_key: key.to_bytes(),
        announced_ips,
        seeds: None,
    };
    
    P2PFrame::send::<OnlineCommand>(
        ctx.clone(),
        &Some(cmd),
        Entity::Node,
        Action::OnLine,
        false,
    ).await?;
    
    tracing::info!("  ✅ Sync: connected to {}", addr);
    Ok(())
}

pub async fn send_tick(
    ctx: Arc<Mutex<Context>>,
    receiver: &str,
    daily_epoch: u64,
    slot: u64,
    _pre_hash: [u8; 32],
) -> anyhow::Result<()> {
    let tick = build_tick_command(ctx.clone(), daily_epoch, slot).await;

    P2PFrame::send::<TickCommand>(ctx.clone(), &Some(tick), Entity::Node, Action::Tick, false)
        .await?;

    tracing::info!("Sent Tick to {}: daily_epoch={}, slot={}", receiver, daily_epoch, slot);
    Ok(())
}