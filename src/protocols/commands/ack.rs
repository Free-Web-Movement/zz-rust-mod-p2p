use std::net::SocketAddr;
use std::sync::Arc;

use aex::{
    connection::{context::Context, node::Node},
    tcp::types::Codec,
};
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use crate::protocols::{command::P2PCommand, command::{Action, Entity}, frame::P2PFrame};
use crate::protocols::commands::online::OnlineCommand;

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct SeedRecord {
    pub address: String,
    pub first_seen: i64,
}

impl SeedRecord {
    pub fn new(address: String) -> Self {
        Self {
            address,
            first_seen: chrono::Utc::now().timestamp(),
        }
    }

    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::default();
        hasher.update(&self.address);
        hasher.update(&self.first_seen.to_le_bytes());
        hasher.finalize().into()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct SeedsCommand {
    pub seeds: Vec<SeedRecord>,
    pub hash: [u8; 32],
}

impl SeedsCommand {
    pub fn new(seeds: Vec<SeedRecord>) -> Self {
        let mut hasher = Sha256::default();
        let mut sorted = seeds.clone();
        sorted.sort_by(|a, b| a.address.cmp(&b.address));
        for seed in &sorted {
            hasher.update(&seed.address);
            hasher.update(&seed.first_seen.to_le_bytes());
        }
        let hash: [u8; 32] = hasher.finalize().into();
        Self { seeds, hash }
    }

    pub fn verify(&self) -> bool {
        let mut hasher = Sha256::default();
        let mut sorted = self.seeds.clone();
        sorted.sort_by(|a, b| a.address.cmp(&b.address));
        for seed in &sorted {
            hasher.update(&seed.address);
            hasher.update(&seed.first_seen.to_le_bytes());
        }
        let computed: [u8; 32] = hasher.finalize().into();
        computed == self.hash
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct OnlineAckCommand {
    pub session_id: Vec<u8>,
    pub address: String,
    pub node: Node,
    pub ephemeral_public_key: [u8; 32],
    pub announced_ips: Vec<String>,
    pub seeds: Option<SeedsCommand>,
}

impl Codec for OnlineAckCommand {}

pub async fn onlineack_handler(ctx: Arc<Mutex<Context>>, frame: P2PFrame, cmd: P2PCommand) {
    println!(
        "✅ Node OnlineAck received from {} nonce={}",
        frame.body.address, frame.body.nonce
    );

    println!("received ack: {:?}", cmd.data);
    let ack: OnlineAckCommand = match Codec::decode(&cmd.data) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("❌ decode OnlineAckCommand failed: {e}");
            return;
        }
    };

    println!("session:id: {:?}", ack.session_id);
    println!("Received external IPs: {:?}", ack.announced_ips);
    println!("Received seeds: {:?}", ack.seeds.as_ref().map(|s| s.seeds.len()));

    let psk = {
        let guard = ctx.lock().await;
        guard.global.paired_session_keys.clone().unwrap()
    };

    {
        let guard = psk.lock().await;
        let _ = guard
            .establish_ends(
                ack.address.as_bytes().to_vec(),
                &ack.ephemeral_public_key.to_vec(),
            )
            .await;
    }
    println!(
        "🔐 Session established with {} (session_id={:?})",
        ack.address, ack.session_id
    );

    // Update endpoint as available in manager - mark as server (inbound)
    let peer_addr: SocketAddr = frame.body.address.parse().unwrap();
    {
        let guard = ctx.lock().await;
        guard.global.manager.update(peer_addr, false, Some(ctx.clone()));
    }
    println!("Updated peer {} as inbound in manager", peer_addr);

    // Store the announced IPs from peer as external seeds
    for ip in ack.announced_ips {
        if ip != "0.0.0.0" && !ip.starts_with("127.") {
            let seed_addr: SocketAddr = format!("{}:0", ip).parse().unwrap_or(peer_addr);
            println!("Storing external IP from peer: {} -> {}", ip, seed_addr);
        }
    }

    // Process seeds consensus from ack - auto connect to new seeds
    if let Some(ref seeds_cmd) = ack.seeds {
        if seeds_cmd.verify() {
            let guard = ctx.lock().await;
            let manager = guard.global.manager.clone();
            let current_entries: Vec<SocketAddr> = manager.get_all_entries();
            let current_addrs: Vec<String> = current_entries.iter().map(|a| a.to_string()).collect();

            println!("🔄 Merging seeds from ack, hash={:?}", seeds_cmd.hash);

            // Calculate new seeds to connect to
            let new_seeds: Vec<String> = seeds_cmd.seeds
                .iter()
                .filter(|s| !current_addrs.contains(&s.address))
                .map(|s| s.address.clone())
                .collect();

            if new_seeds.is_empty() {
                println!("  ✓ No new seeds to connect to");
            } else {
                println!("  🌱 Auto connecting to new seeds: {:?}", new_seeds);
                
                for seed_addr_str in new_seeds {
                    if let Ok(seed_addr) = seed_addr_str.parse::<SocketAddr>() {
                        let addr_str = seed_addr_str.clone();
                        let ctx_owned = ctx.clone();
                        tokio::spawn(async move {
                            if let Err(e) = connect_to_new_peer(ctx_owned, seed_addr).await {
                                eprintln!("  ❌ Failed to connect to {}: {:?}", addr_str, e);
                            }
                        });
                    }
                }
            }
        } else {
            eprintln!("❌ Invalid seeds hash in ack!");
        }
    }
}

async fn connect_to_new_peer(ctx: Arc<Mutex<Context>>, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let psk = {
        let guard = ctx.lock().await;
        guard.global.paired_session_keys.clone().unwrap()
    };
    
    let ctx_for_seeds = ctx.clone();
    let (id, key) = {
        let guard = psk.lock().await;
        guard.create(false).await
    };
    
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

    let aex_node = Node::from_system(addr.port(), id.clone(), 1);
    let cmd = OnlineCommand {
        session_id: id,
        node: aex_node,
        ephemeral_public_key: key.to_bytes(),
        announced_ips: vec![],
        seeds: Some(seeds_to_send),
    };
    
    P2PFrame::send::<OnlineCommand>(
        ctx.clone(),
        &Some(cmd),
        Entity::Node,
        Action::OnLine,
        false,
    ).await?;
    
    println!("  ✅ Sent OnLine to {}", addr);
    Ok(())
}
