use std::net::SocketAddr;
use std::sync::Arc;

use aex::{
    connection::{context::Context, node::Node as AexNode, scope::NetworkScope},
    tcp::types::Codec,
};
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use zz_account::address::FreeWebMovementAddress;

use crate::node::Node;
use crate::protocols::{command::P2PCommand, command::{Action, Entity}, frame::P2PFrame};
use crate::protocols::commands::online::OnlineCommand;

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct SeedRecord {
    pub address: String,
    pub node_address: String,
    pub first_seen: i64,
}

impl SeedRecord {
    pub fn new(address: String, node_address: String) -> Self {
        Self {
            address,
            node_address,
            first_seen: chrono::Utc::now().timestamp(),
        }
    }

    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::default();
        hasher.update(&self.node_address);
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
        let mut deduped: Vec<SeedRecord> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for seed in seeds {
            if !seen.contains(&seed.address) {
                seen.insert(seed.address.clone());
                deduped.push(seed);
            }
        }

        let mut hasher = Sha256::default();
        let mut sorted = deduped.clone();
        sorted.sort_by(|a, b| a.node_address.cmp(&b.node_address));
        for seed in &sorted {
            hasher.update(&seed.node_address);
            hasher.update(&seed.first_seen.to_le_bytes());
        }
        let hash: [u8; 32] = hasher.finalize().into();
        Self { seeds: deduped, hash }
    }

    pub fn verify(&self) -> bool {
        let mut hasher = Sha256::default();
        let mut sorted = self.seeds.clone();
        sorted.sort_by(|a, b| a.node_address.cmp(&b.node_address));
        for seed in &sorted {
            hasher.update(&seed.node_address);
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
    pub node: AexNode,
    pub ephemeral_public_key: [u8; 32],
    pub intranet_ips: Vec<String>,
    pub wan_ips: Vec<String>,
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
    println!("Received intranet IPs: {:?}", ack.intranet_ips);
    println!("Received wan IPs: {:?}", ack.wan_ips);
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

    // Register peer node in NodeRegistry
    let peer_addr = {
        let guard = ctx.lock().await;
        guard.addr
    };
    let peer_address = ack.address.clone();
    let scope = NetworkScope::from_ip(&peer_addr.ip());
    {
        let guard = ctx.lock().await;
        if let Some(node) = guard.global.get::<Arc<Node>>().await {
            node.registry.register(peer_address.clone(), peer_addr, scope);
            println!("📝 Registered peer node: {} at {}", peer_address, peer_addr);
        }
    }

    // Store peer's Node info in ConnectionEntry so get_connection_info() can read it
    let peer_node = ack.node.clone();
    let entry_opt = {
        let guard = ctx.lock().await;
        let manager = &guard.global.manager;
        match manager.connections.get(&(peer_addr.ip(), scope)) {
            Some(bi_conn) => match bi_conn.servers.get(&peer_addr) {
                Some(entry_ref) => Some(entry_ref.value().clone()),
                None => None,
            },
            None => None,
        }
    };
    if let Some(entry) = entry_opt {
        entry.update_node(peer_node).await;
    }

    // Update endpoint as available in manager - mark as server (inbound)
    {
        let guard = ctx.lock().await;
        guard.global.manager.update(peer_addr, false, Some(ctx.clone()));
    }
    println!("Updated peer {} as inbound in manager", peer_addr);

    // Store the announced IPs from peer as external seeds
    for ip in ack.intranet_ips.iter().chain(ack.wan_ips.iter()) {
        let is_loopback = match ip.parse::<std::net::IpAddr>() {
            Ok(addr) => addr.is_loopback(),
            Err(_) => false,
        };
        if !is_loopback {
            let seed_addr: SocketAddr = format!("{}:{}", ip, peer_addr.port()).parse().unwrap_or(peer_addr);
            let node = {
                let guard = ctx.lock().await;
                guard.global.get::<Arc<Node>>().await
            };
            if let Some(n) = node {
                n.registry.add_seed(&peer_address, seed_addr);
            }
            println!("Storing external IP from peer: {} -> {}", ip, seed_addr);
        }
    }

    // Process seeds consensus from ack - auto connect to new seeds
    if let Some(ref seeds_cmd) = ack.seeds {
        if seeds_cmd.verify() {
            let guard = ctx.lock().await;
            let gctx = guard.global.clone();
            drop(guard);

            println!("🔄 Merging seeds from ack, hash={:?}", seeds_cmd.hash);

            // Register received seeds in NodeRegistry
            let node = gctx.get::<Arc<Node>>().await;
            if let Some(ref node) = node {
                let before_count = node.registry.get_all_seeds().len();
                
                for seed in &seeds_cmd.seeds {
                    if let Ok(seed_addr) = seed.address.parse::<SocketAddr>() {
                        let scope = NetworkScope::from_ip(&seed_addr.ip());
                        node.registry.register(seed.node_address.clone(), seed_addr, scope);
                        println!("  + Registered seed from ack: {} (node: {})", seed.address, seed.node_address);
                    }
                }

                let after_count = node.registry.get_all_seeds().len();
                
                // Broadcast updated seeds to all peers if we learned new ones
                if after_count > before_count {
                    let ctx_for_broadcast = ctx.clone();
                    
                    let all_seeds: Vec<SeedRecord> = node.registry.get_all_seeds()
                        .into_iter()
                        .map(|(s, na)| SeedRecord::new(s.to_string(), na))
                        .collect();
                    let seeds_to_broadcast = SeedsCommand::new(all_seeds);
                    
                    tokio::spawn(async move {
                        broadcast_seeds_to_peers(ctx_for_broadcast, &seeds_to_broadcast).await;
                    });
                }

                // Connect to new nodes (node-based dedup)
                for seed in &seeds_cmd.seeds {
                    // Skip if this node is already connected
                    if node.registry.is_connected(&seed.node_address) {
                        println!("⏭️ Skipping seed {} - node {} already connected", seed.address, seed.node_address);
                        continue;
                    }

                    if let Ok(seed_addr) = seed.address.parse::<SocketAddr>() {
                        let addr_str = seed.address.clone();
                        let ctx_owned = ctx.clone();
                        let reg_clone = node.registry.clone();
                        let node_addr = seed.node_address.clone();
                        tokio::spawn(async move {
                            println!("🔗 Attempting connection to {} (node: {})", addr_str, node_addr);
                            if connect_to_new_peer(ctx_owned, seed_addr).await.is_ok() {
                                reg_clone.mark_connected(&node_addr, true);
                                println!("✅ Connected to node {} via {}", node_addr, addr_str);
                            } else {
                                eprintln!("  ❌ Failed to connect to {}: seed of node {}", addr_str, node_addr);
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

pub async fn broadcast_seeds_to_peers(ctx: Arc<Mutex<Context>>, seeds: &SeedsCommand) {
    let gctx = {
        let guard = ctx.lock().await;
        guard.global.clone()
    };

    let self_node_id = gctx.local_node.read().await.id.clone();
    let address = gctx.get::<FreeWebMovementAddress>().await.unwrap();

    let cmd = OnlineCommand {
        session_id: vec![],
        node: AexNode::from_system(0, self_node_id.clone(), 1),
        ephemeral_public_key: [0u8; 32],
        intranet_ips: vec![],
        wan_ips: vec![],
        seeds: Some(seeds.clone()),
    };

    let cmd_bytes = Codec::encode(&cmd);
    let p2p_cmd = P2PCommand::new(Entity::Node, Action::OnLine, cmd_bytes);
    let frame = P2PFrame::build(&address, p2p_cmd, 1).await.unwrap();
    let seed_count = seeds.seeds.len();

    let manager = gctx.manager.clone();
    let frame_bytes = Codec::encode(&frame);

    manager
        .forward(|entries| async move {
            for entry in entries {
                if let Some(peer_ctx) = &entry.context {
                    let mut guard = peer_ctx.lock().await;
                    if let Some(writer) = &mut guard.writer {
                        if let Err(e) = writer.write_all(&frame_bytes).await {
                            eprintln!("  ❌ Failed to broadcast seeds: {:?}", e);
                        } else {
                            let _ = writer.flush().await;
                        }
                    }
                }
            }
        })
        .await;

    println!("  📢 Broadcast seeds ({} seeds) to all peers", seed_count);
}

pub async fn connect_to_new_peer(ctx: Arc<Mutex<Context>>, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let gctx = {
        let guard = ctx.lock().await;
        guard.global.clone()
    };

    let psk = gctx.paired_session_keys.clone().unwrap();
    let (id, key) = {
        let guard = psk.lock().await;
        guard.create(false).await
    };

    let (seeds_to_send, self_node_id) = {
        let self_node_id = gctx.local_node.read().await.id.clone();
        let seeds = if let Some(node) = gctx.get::<Arc<Node>>().await {
            let all_seeds: Vec<SeedRecord> = node.registry
                .get_all_seeds()
                .into_iter()
                .map(|(s, na)| SeedRecord::new(s.to_string(), na))
                .collect();
            SeedsCommand::new(all_seeds)
        } else {
            SeedsCommand::new(vec![])
        };
        (seeds, self_node_id)
    };

    let aex_node = AexNode::from_system(addr.port(), self_node_id, 1);
    let (intranet_ips, wan_ips) = super::online::get_all_ips();
    let cmd = Arc::new(OnlineCommand {
        session_id: id,
        node: aex_node,
        ephemeral_public_key: key.to_bytes(),
        intranet_ips,
        wan_ips,
        seeds: Some(seeds_to_send),
    });

    let gctx_clone = gctx.clone();
    let cmd_clone = cmd.clone();

    match gctx.manager.clone().connect::<P2PFrame, P2PCommand, _, _>(
        addr,
        gctx.clone(),
        move |new_ctx| {
            let cmd = cmd_clone.clone();
            let g = gctx_clone.clone();
            Box::pin(async move {
                if let Err(e) = P2PFrame::send::<OnlineCommand>(
                    new_ctx.clone(),
                    &Some((*cmd).clone()),
                    Entity::Node,
                    Action::OnLine,
                    false,
                ).await {
                    tracing::error!("❌ Failed to send OnlineCommand: {:?}", e);
                    return;
                }
                if let Some(router) = aex::connection::context::get_tcp_router::<P2PFrame, P2PCommand>(&g.routers) {
                    let _ = router.handle(new_ctx).await;
                }
            })
        },
        Some(10),
    ).await {
        Ok(_) => {
            tracing::info!("  ✅ Connected to peer via outbound: {}", addr);
            Ok(())
        }
        Err(e) => {
            tracing::warn!("  ⚠️  connect_to_new_peer {} failed: {:?}", addr, e);
            Err(format!("connect_to_new_peer {} failed: {:?}", addr, e).into())
        }
    }
}
