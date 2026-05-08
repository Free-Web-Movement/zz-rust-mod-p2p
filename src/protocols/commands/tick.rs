use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::SystemTime;

use aex::connection::context::Context;
use aex::connection::scope::NetworkScope;
use aex::tcp::types::Codec;
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use crate::node::Node;
use crate::protocols::command::P2PCommand;
use crate::protocols::command::{Action, Entity};
use crate::protocols::commands::ack::{broadcast_seeds_to_peers, SeedRecord, SeedsCommand};
use crate::protocols::commands::node_registry::NodeRegistry;
use crate::protocols::frame::P2PFrame;

pub const WITNESS_RING_STABLE_ROUNDS: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct TickCommand {
    pub daily_epoch: u64,
    pub slot: u64,
    pub challenge: Vec<u8>,
    pub seeds: Option<SeedsCommand>,
    pub ring_hash: [u8; 32],
}

impl Codec for TickCommand {}

async fn build_tick_command(ctx: Arc<Mutex<Context>>) -> TickCommand {
    let seed_records = {
        let guard = ctx.lock().await;
        if let Some(node) = guard.global.get::<Arc<Node>>().await {
            get_witness_ring_from_registry(&node.registry).await
        } else {
            vec![]
        }
    };

    let ring_hash = compute_witness_ring_hash(&seed_records);

    TickCommand {
        daily_epoch: 0,
        slot: 0,
        challenge: vec![],
        seeds: if seed_records.is_empty() {
            None
        } else {
            Some(SeedsCommand::new(seed_records))
        },
        ring_hash,
    }
}

pub fn compute_witness_ring_hash(seeds: &[SeedRecord]) -> [u8; 32] {
    let mut sorted: Vec<String> = seeds.iter().map(|s| s.address.clone()).collect();
    sorted.sort();
    
    let mut hasher = Sha256::default();
    for addr in &sorted {
        hasher.update(addr.as_bytes());
    }
    hasher.finalize().into()
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
        "✅ Tick from {}: ring_hash={:?}",
        frame.body.address,
        &tick.ring_hash[..8]
    );

    if let Some(ref seeds_cmd) = tick.seeds {
        if seeds_cmd.verify() {
            sync_witness_ring(ctx.clone(), seeds_cmd).await;
        } else {
            eprintln!("❌ Invalid seeds hash in tick!");
        }
    }

    let local_ring = get_witness_ring(ctx.clone()).await;
    let local_hash = compute_witness_ring_hash(&local_ring);
    let ring_size = local_ring.len();

    let merged_seeds = SeedsCommand::new(local_ring.clone());
    broadcast_seeds_to_peers(ctx.clone(), &merged_seeds).await;

    let is_stable = local_hash == tick.ring_hash;

    if is_stable {
        let prev_hash: Option<[u8; 32]> = {
            let guard = ctx.lock().await;
            guard.global.get::<[u8; 32]>().await
        };

        let stable = if prev_hash == Some(local_hash) {
            let guard = ctx.lock().await;
            let count = guard.global.get::<u32>().await.unwrap_or(0);
            let new_count = count + 1;
            guard.global.set(new_count).await;
            new_count >= WITNESS_RING_STABLE_ROUNDS
        } else {
            let guard = ctx.lock().await;
            guard.global.set(1u32).await;
            guard.global.set(local_hash).await;
            false
        };

        if stable {
            tracing::info!("🔒 Witness ring STABLE: {} nodes, hash={:?}", ring_size, &local_hash[..8]);
            lock_witness_ring(ctx.clone(), &local_ring, local_hash).await;
        }
    }

    let response = TickCommand {
        daily_epoch: tick.daily_epoch,
        slot: tick.slot,
        challenge: vec![],
        seeds: Some(merged_seeds.clone()),
        ring_hash: merged_seeds.hash,
    };

    let _ = P2PFrame::send::<TickCommand>(
        ctx.clone(),
        &Some(response),
        Entity::Node,
        Action::TickAck,
        false,
    ).await;

    tracing::info!(
        "📊 Witness ring: {} nodes, hash={:?}, stable={}",
        ring_size,
        &local_hash[..8],
        is_stable
    );
}

async fn lock_witness_ring(ctx: Arc<Mutex<Context>>, ring: &[SeedRecord], hash: [u8; 32]) {
    let gctx = {
        let guard = ctx.lock().await;
        guard.global.clone()
    };

    let now = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    for (i, seed) in ring.iter().enumerate() {
        let witness = WitnessEntry {
            address: seed.node_address.clone(),
            seed_addr: seed.address.clone(),
            weight: 1,
            status: "active".to_string(),
            last_tick_ts: now,
            rank: i as u32,
        };

        let existing = gctx.get::<Vec<WitnessEntry>>().await.unwrap_or_default();
        let pos = existing.iter().position(|w| w.address == witness.address);
        
        let updated = if let Some(p) = pos {
            let mut v = existing;
            v[p] = witness;
            v
        } else {
            let mut v = existing;
            v.push(witness);
            v
        };

        gctx.set(updated).await;
    }

    gctx.set(hash).await;

    let total_nodes = ring.len();
    let total_reward: u64 = 1000; 
    let per_witness = if total_nodes > 0 { total_reward / total_nodes as u64 } else { 0 };

    let mut balances: Vec<BalanceEntry> = gctx.get::<Vec<BalanceEntry>>().await.unwrap_or_default();
    for seed in ring {
        let existing = balances.iter().position(|b| b.address == seed.node_address);
        if let Some(pos) = existing {
            balances[pos].balance += per_witness;
        } else {
            balances.push(BalanceEntry {
                address: seed.node_address.clone(),
                balance: per_witness,
                updated_at: now,
            });
        }
    }
    gctx.set(balances).await;

    tracing::info!(
        "💰 Balance distribution: {} witnesses, {} per witness",
        total_nodes,
        per_witness
    );
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessEntry {
    pub address: String,
    pub seed_addr: String,
    pub weight: u64,
    pub status: String,
    pub last_tick_ts: u64,
    pub rank: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceEntry {
    pub address: String,
    pub balance: u64,
    pub updated_at: u64,
}

async fn sync_witness_ring(ctx: Arc<Mutex<Context>>, seeds_cmd: &SeedsCommand) {
    let node = {
        let guard = ctx.lock().await;
        guard.global.get::<Arc<Node>>().await
    };

    if let Some(node) = node {
        let local_addrs: HashSet<String> = node.registry.get_all_seeds()
            .iter()
            .map(|(s, _)| s.to_string())
            .collect();

        for seed in &seeds_cmd.seeds {
            if !local_addrs.contains(&seed.address) {
                if let Ok(seed_addr) = seed.address.parse::<SocketAddr>() {
                    let scope = NetworkScope::from_ip(&seed_addr.ip());
                    node.registry.register(seed.node_address.clone(), seed_addr, scope);
                    println!("  + Tick registered new seed: {} (node: {})", seed.address, seed.node_address);
                }
            }
        }

        for seed in &seeds_cmd.seeds {
            if !node.registry.is_connected(&seed.node_address) {
                if let Ok(seed_addr) = seed.address.parse::<SocketAddr>() {
                    let ctx_spawn = ctx.clone();
                    let addr = seed_addr;
                    let reg_clone = node.registry.clone();
                    let node_addr = seed.node_address.clone();
                    tokio::spawn(async move {
                        if crate::protocols::commands::ack::connect_to_new_peer(ctx_spawn, addr).await.is_ok() {
                            reg_clone.mark_connected(&node_addr, true);
                            println!("  ✅ Tick connected to node {} via {}", node_addr, addr);
                        }
                    });
                }
            }
        }
    }
}

async fn get_witness_ring_from_registry(reg: &NodeRegistry) -> Vec<SeedRecord> {
    reg.get_all_seeds()
        .into_iter()
        .map(|(s, na)| SeedRecord::new(s.to_string(), na))
        .collect()
}

async fn get_witness_ring(ctx: Arc<Mutex<Context>>) -> Vec<SeedRecord> {
    let node = {
        let guard = ctx.lock().await;
        guard.global.get::<Arc<Node>>().await
    };

    if let Some(ref node) = node {
        get_witness_ring_from_registry(&node.registry).await
    } else {
        vec![]
    }
}

pub async fn send_tick(
    ctx: Arc<Mutex<Context>>,
    receiver: &str,
    daily_epoch: u64,
    slot: u64,
    _pre_hash: [u8; 32],
) -> anyhow::Result<()> {
    let tick = build_tick_command(ctx.clone()).await;

    P2PFrame::send::<TickCommand>(ctx.clone(), &Some(tick), Entity::Node, Action::Tick, false)
        .await?;

    tracing::info!("Sent Tick to {}: daily_epoch={}, slot={}", receiver, daily_epoch, slot);
    Ok(())
}
