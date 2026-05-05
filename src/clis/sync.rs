use aex::connection::{global::GlobalContext, node::Node};
use std::{net::SocketAddr, sync::Arc};
use sha2::{Digest, Sha256};

use crate::protocols::{
    command::{Action, Entity, P2PCommand},
    commands::online::{get_all_ips, OnlineCommand},
    frame::P2PFrame,
};
use crate::protocols::commands::ack::{SeedRecord, SeedsCommand};

pub const SYNC_ROUNDS_PER_TICK: u32 = 3;
pub const SYNC_INTERVAL_MS: u64 = 200;

pub async fn handle(args: Vec<String>, context: Arc<GlobalContext>) {
    let target_addrs: Vec<SocketAddr> = if args.is_empty() {
        context.manager.get_all_entries()
    } else {
        args.iter().filter_map(|a| a.parse::<SocketAddr>().ok()).collect()
    };

    if target_addrs.is_empty() {
        println!("Usage: sync [<ip>:<port> ...]");
        println!("  Without args: sync with all connected peers");
        println!("  With args: sync with specified peers");
        return;
    };

    let mut prev_hash: [u8; 32] = [0u8; 32];
    let mut stable_count = 0;

    println!("🔄 Sync start: {} seeds", target_addrs.len());

    for round in 0..SYNC_ROUNDS_PER_TICK {
        let mut pending = Vec::new();
        
        for addr in &target_addrs {
            if !is_connected(&context, *addr) {
                let ctx = context.clone();
                let addr = *addr;
                pending.push(tokio::spawn(async move {
                    connect_to_peer_sync(ctx, addr).await
                }));
            }
        }

        for h in pending {
            let _ = h.await;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(SYNC_INTERVAL_MS)).await;

        let current_seeds: Vec<String> = get_connected_seeds(&context);
        let new_hash = compute_witness_hash(&current_seeds);
        
        println!("  📊 Round {}: {} connected, hash={:?}", round + 1, current_seeds.len(), &new_hash[..8]);

        if new_hash == prev_hash && !current_seeds.is_empty() {
            stable_count += 1;
            if stable_count >= 2 {
                println!("✅ Sync complete: {} connected", current_seeds.len());
                return;
            }
        } else {
            stable_count = 0;
        }

        prev_hash = new_hash;
    }

    let final_seeds = get_connected_seeds(&context);
    println!("⚠️ Final: {:?}", final_seeds);
}

fn is_connected(ctx: &Arc<GlobalContext>, addr: SocketAddr) -> bool {
    ctx.manager.get_all_entries().contains(&addr)
}

fn get_connected_seeds(ctx: &Arc<GlobalContext>) -> Vec<String> {
    let mut addrs: Vec<String> = ctx.manager.get_all_entries()
        .iter()
        .map(|a| a.to_string())
        .collect();
    addrs.sort();
    addrs
}

async fn connect_to_peer_sync(
    context: Arc<GlobalContext>,
    addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    
    let _ = context.manager.clone().connect::<P2PFrame, P2PCommand, _, _>(
        addr,
        context.clone(),
        move |ctx| {
            let tx = tx;
            let ctx_for_seeds = ctx.clone();
            Box::pin(async move {
                let psk = {
                    let guard = ctx.lock().await;
                    guard.global.paired_session_keys.clone().unwrap()
                };
                let (id, key) = {
                    let guard = psk.lock().await;
                    guard.create(false).await
                };
                let aex_node = Node::from_system(addr.port(), id.clone(), 1);

                // Build seeds to send
                let seeds_to_send = {
                    let guard = ctx_for_seeds.lock().await;
                    let connected: Vec<String> = guard.global.manager.get_all_entries()
                        .iter()
                        .map(|a| a.to_string())
                        .collect();
                    let self_addr = guard.global.addr.to_string();
                    drop(guard);

                    let mut all_seeds: Vec<SeedRecord> = connected
                        .iter()
                        .filter(|s| *s != &self_addr)
                        .map(|a| SeedRecord::new(a.clone()))
                        .collect();
                    all_seeds.push(SeedRecord::new(self_addr));
                    SeedsCommand::new(all_seeds)
                };

                let cmd = OnlineCommand {
                    session_id: id,
                    node: aex_node,
                    ephemeral_public_key: key.to_bytes(),
                    announced_ips: get_all_ips(),
                    seeds: Some(seeds_to_send),
                };
                let _ = P2PFrame::send::<OnlineCommand>(
                    ctx.clone(),
                    &Some(cmd),
                    Entity::Node,
                    Action::OnLine,
                    false,
                ).await;
                let _ = tx.send(());
            })
        },
        Some(5),
    ).await;

    rx.await.map_err(|e| e.into())
}

fn compute_witness_hash(seeds: &[String]) -> [u8; 32] {
    let mut hasher = Sha256::default();
    for seed in seeds {
        hasher.update(seed.as_bytes());
    }
    hasher.finalize().into()
}