use std::sync::Arc;

use aex::connection::context::Context;
use aex::connection::node::Node;
use aex::connection::scope::NetworkScope;
use aex::tcp::types::Codec;
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use zz_account::address::FreeWebMovementAddress;

use crate::node::Node as P2pNode;
use crate::protocols::command::P2PCommand;
use crate::protocols::command::{Action, Entity};
use crate::protocols::commands::ack::{OnlineAckCommand, SeedRecord, SeedsCommand};
use crate::protocols::frame::P2PFrame;

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct OnlineCommand {
    pub session_id: Vec<u8>,
    pub node: Node,
    pub ephemeral_public_key: [u8; 32],
    pub intranet_ips: Vec<String>,
    pub wan_ips: Vec<String>,
    pub seeds: Option<SeedsCommand>,
}

impl Codec for OnlineCommand {}

pub async fn online_handler(ctx: Arc<Mutex<Context>>, frame: P2PFrame, cmd: P2PCommand) {
    println!("inside online handler!");
    let online: OnlineCommand = match Codec::decode(&cmd.data) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("❌ decode OnlineCommand failed: {e}");
            return ();
        }
    };
    println!(
        "✅ Node Online: addr={}, nonce={}",
        frame.body.address, frame.body.nonce
    );

    println!("received session_id: {:?}", online.session_id);
    println!("intranet IPs: {:?}", online.intranet_ips);
    println!("wan IPs: {:?}", online.wan_ips);
    println!(
        "received seeds: {:?}",
        online.seeds.as_ref().map(|s| s.seeds.len())
    );

    // Handle seed-only gossip (empty session_id means broadcast from existing peer)
    if online.session_id.is_empty() {
        // Store peer's Node info in ConnectionEntry so get_connection_info()
        // can read the node_id (important for return connections).
        let gossip_peer_node = online.node.clone();
        {
            let guard = ctx.lock().await;
            let peer_addr = guard.addr;
            let scope = aex::connection::scope::NetworkScope::from_ip(&peer_addr.ip());
            let manager = &guard.global.manager;
            if let Some(bi_conn) = manager.connections.get(&(peer_addr.ip(), scope)) {
                // The connection could be a server (we initiated outbound) or client (inbound)
                if let Some(entry_ref) = bi_conn.servers.get(&peer_addr) {
                    entry_ref.value().update_node(gossip_peer_node).await;
                } else if let Some(entry_ref) = bi_conn.clients.get(&peer_addr) {
                    entry_ref.value().update_node(gossip_peer_node).await;
                }
            }
        }

        let gctx = {
            let guard = ctx.lock().await;
            guard.global.clone()
        };

        // Register the gossiping peer itself (it sent us its Node info in online.node)
        if let Some(node) = gctx.get::<Arc<P2pNode>>().await {
            let gossip_sock = {
                let guard = ctx.lock().await;
                guard.addr
            };
            // Use the peer's advertised listening port, not the TCP source port
            let listen_addr = std::net::SocketAddr::new(gossip_sock.ip(), online.node.port);
            let gossip_scope = aex::connection::scope::NetworkScope::from_ip(&listen_addr.ip());
            node.registry
                .register(frame.body.address.clone(), listen_addr, gossip_scope);
        }

        if let Some(ref peer_seeds) = online.seeds {
            if peer_seeds.verify() {
                println!(
                    "📥 Received seed gossip, merging {} seeds",
                    peer_seeds.seeds.len()
                );

                // Register peer nodes from gossip seeds and connect (node-based dedup)
                let node = gctx.get::<Arc<P2pNode>>().await;
                if let Some(node) = node {
                    let reg = &node.registry;
                    let before_count = reg.get_all_seeds().len();

                    for seed in &peer_seeds.seeds {
                        // Register the node if not already known
                        if !reg.is_registered(&seed.node_address) {
                            if let Ok(seed_addr) = seed.address.parse::<std::net::SocketAddr>() {
                                let scope = NetworkScope::from_ip(&seed_addr.ip());
                                reg.register(seed.node_address.clone(), seed_addr, scope);
                            }
                        }

                        // Skip if node already connected
                        if reg.is_connected(&seed.node_address) {
                            continue;
                        }

                        // Skip if seed is our own address (prevent self-connect loop)
                        if let Some(local_addr) = gctx.get::<FreeWebMovementAddress>().await {
                            if seed.node_address == local_addr.to_string() {
                                tracing::info!("⏭️ Skipping self-connect to {}", seed.address);
                                continue;
                            }
                        }

                        // Tiebreaker: only the node with the smaller address initiates the connection.
                        // This prevents the race where both sides simultaneously connect and establish
                        // different session keys for the same peer pair.
                        if let Some(local_addr) = gctx.get::<FreeWebMovementAddress>().await {
                            if local_addr.to_string() > seed.node_address {
                                tracing::info!(
                                    "⏭️ Tiebreaker: {} > {}, letting lower address initiate",
                                    local_addr,
                                    seed.node_address
                                );
                                continue;
                            }
                        }

                        if let Ok(seed_addr) = seed.address.parse::<std::net::SocketAddr>() {
                            let ctx_owned = ctx.clone();
                            let addr_str = seed.address.clone();
                            let reg_clone = reg.clone();
                            let node_addr = seed.node_address.clone();
                            tokio::spawn(async move {
                                if super::ack::connect_to_new_peer(ctx_owned, seed_addr)
                                    .await
                                    .is_ok()
                                {
                                    reg_clone.mark_connected(&node_addr, true);
                                } else {
                                    eprintln!(
                                        "  ❌ Failed to connect to gossiped seed {}",
                                        addr_str
                                    );
                                }
                            });
                        }
                    }

                    // Propagate to other peers if we learned new seeds
                    let after_count = reg.get_all_seeds().len();
                    if after_count > before_count {
                        let ctx_for_broadcast = ctx.clone();
                        let all_seeds: Vec<crate::protocols::commands::ack::SeedRecord> = reg
                            .get_all_seeds()
                            .into_iter()
                            .map(|(s, na)| {
                                crate::protocols::commands::ack::SeedRecord::new(s.to_string(), na)
                            })
                            .collect();
                        let seeds_to_broadcast =
                            crate::protocols::commands::ack::SeedsCommand::new(all_seeds);

                        tokio::spawn(async move {
                            super::ack::broadcast_seeds_to_peers(
                                ctx_for_broadcast,
                                &seeds_to_broadcast,
                            )
                            .await;
                        });
                    }
                }
            }
        }
        return;
    }

    // ============================================================
    // 节点去重：同一 node.id 只能有一个 inbound 连接
    // 但回连（return connection）来自对端且方向不同
    // ============================================================
    let is_return_conn: bool;
    {
        let gctx = {
            let guard = ctx.lock().await;
            guard.global.clone()
        };
        let node = gctx.get::<Arc<P2pNode>>().await;
        if let Some(node) = node {
            is_return_conn = !node.registry.try_connect(&frame.body.address);
            if is_return_conn {
                tracing::warn!(
                    "⚠️ Node {} is already connected (return connection), skipping key exchange",
                    frame.body.address
                );
            } else {
                tracing::info!(
                    "✅ Node {} connected (1st connection, accepted)",
                    frame.body.address
                );
            }

            // Register peer as a seed using its listening port (from online.node.port),
            // NOT the ephemeral TCP source port (guard.addr.port). Seeds must always
            // point to a reachable listener, not a transient connection endpoint.
            let peer_sock = {
                let guard = ctx.lock().await;
                guard.addr
            };
            let listen_addr = std::net::SocketAddr::new(peer_sock.ip(), online.node.port);
            let scope = aex::connection::scope::NetworkScope::from_ip(&listen_addr.ip());
            // Both first and return connections register their direction.
            // register_with_direction uses a HashSet, so duplicate directions
            // for the same (peer, seed) are idempotent.
            node.registry.register_with_direction(
                frame.body.address.clone(),
                listen_addr,
                scope,
                crate::protocols::commands::node_registry::ConnectionDirection::Inbound,
            );
        } else {
            is_return_conn = false;
        }
    }

    // Store peer's FreeWebMovement address in connection context for encryption key lookup
    {
        let mut guard = ctx.lock().await;
        guard.set(frame.body.address.clone());
    }

    // Store peer's Node info in ConnectionEntry so get_connection_info() can read it
    let peer_node = online.node.clone();
    let entry_opt = {
        let guard = ctx.lock().await;
        let peer_addr = guard.addr;
        let scope = aex::connection::scope::NetworkScope::from_ip(&peer_addr.ip());
        let manager = &guard.global.manager;
        let key = (peer_addr.ip(), scope);
        tracing::debug!(
            "🔍 ENTRY_LOOKUP: peer_addr={}, scope={:?}, key={:?}",
            peer_addr,
            scope,
            key
        );
        match manager.connections.get(&key) {
            Some(bi_conn) => {
                let in_clients = bi_conn.clients.contains_key(&peer_addr);
                let in_servers = bi_conn.servers.contains_key(&peer_addr);
                tracing::debug!(
                    "🔍 ENTRY_LOOKUP: bucket found, clients_contains={}, servers_contains={}",
                    in_clients,
                    in_servers
                );
                match bi_conn.clients.get(&peer_addr) {
                    Some(entry_ref) => Some(entry_ref.value().clone()),
                    None => match bi_conn.servers.get(&peer_addr) {
                        Some(entry_ref) => Some(entry_ref.value().clone()),
                        None => None,
                    },
                }
            }
            None => {
                tracing::warn!("🔍 ENTRY_LOOKUP: bucket NOT FOUND for key={:?}", key);
                None
            }
        }
    };
    let peer_node_id_debug = String::from_utf8_lossy(&peer_node.id).to_string();
    if let Some(entry) = entry_opt {
        entry.update_node(peer_node).await;
        tracing::debug!(
            "🔍 ENTRY_LOOKUP: update_node OK for node_id={:?}",
            peer_node_id_debug
        );
    } else {
        tracing::warn!(
            "🔍 ENTRY_LOOKUP: entry NOT FOUND, update_node SKIPPED for node_id={:?}",
            peer_node_id_debug
        );
    }

    let psk = {
        let ctx = ctx.lock().await;
        ctx.global.paired_session_keys.clone().unwrap()
    };

    let addr_debug = frame.body.address.clone();
    let local_addr_for_key = {
        let guard = ctx.lock().await;
        guard
            .global
            .get::<FreeWebMovementAddress>()
            .await
            .unwrap()
            .to_string()
    };
    let ephemeral_public = if is_return_conn {
        // Return connection: check if a session key already exists for this peer.
        // If not, the first connection's key exchange was lost in a race (both
        // sides initiated simultaneously via seed gossip). Perform the exchange
        // now rather than sending a zero-key ack that leaves this peer without
        // a session key for the initiator.
        let guard = psk.lock().await;
        match guard
            .establish_begins(
                frame.body.address.as_bytes().to_vec(),
                local_addr_for_key.as_bytes().to_vec(),
                &online.ephemeral_public_key.to_vec(),
            )
            .await
        {
            Ok(Some(pk)) => {
                tracing::info!("🔑 establish_begins OK for address='{}'", addr_debug);
                pk
            }
            Ok(None) => {
                // Key already exists for this peer (or DH failed) — skip exchange
                tracing::info!(
                    "🔑 establish_begins skipped for address='{}' (key exists)",
                    addr_debug
                );
                let zero_key = x25519_dalek::PublicKey::from([0u8; 32]);
                zero_key
            }
            Err(e) => {
                tracing::error!(
                    "❌ establish_begins error for address='{}': {:?}",
                    addr_debug,
                    e
                );
                return;
            }
        }
    } else {
        let guard = psk.lock().await;
        match guard
            .establish_begins(
                frame.body.address.as_bytes().to_vec(),
                local_addr_for_key.as_bytes().to_vec(),
                &online.ephemeral_public_key.to_vec(),
            )
            .await
        {
            Ok(Some(pk)) => {
                tracing::info!("🔑 establish_begins OK for address='{}'", addr_debug);
                pk
            }
            Ok(None) => {
                tracing::info!(
                    "🔑 establish_begins skipped for address='{}' (key exists)",
                    addr_debug
                );
                let zero_key = x25519_dalek::PublicKey::from([0u8; 32]);
                zero_key
            }
            Err(e) => {
                tracing::error!(
                    "❌ establish_begins error for address='{}': {:?}",
                    addr_debug,
                    e
                );
                return;
            }
        }
    };

    let address: FreeWebMovementAddress = {
        let ctx = ctx.lock().await;
        ctx.global.get().await.expect("Expect Address be set!")
    };

    let local_node = {
        let ctx = ctx.lock().await;
        ctx.global.local_node.clone()
    };

    let node = {
        let guard = local_node.write().await;
        guard.clone()
    };

    let (intranet_ips, wan_ips) = get_all_ips();
    println!("Announcing intranet IPs: {:?}", intranet_ips);
    println!("Announcing wan IPs: {:?}", wan_ips);

    // Build seeds from NodeRegistry
    let seeds_to_send = {
        let guard = ctx.lock().await;
        let _self_addr = guard.global.addr.to_string();
        drop(guard);

        // Merge peer's seeds into NodeRegistry
        if let Some(ref peer_seeds) = online.seeds {
            if peer_seeds.verify() {
                println!(
                    "🔄 Merging peer's seeds: {} seeds, hash={:?}",
                    peer_seeds.seeds.len(),
                    peer_seeds.hash
                );
                let node = ctx.lock().await.global.get::<Arc<P2pNode>>().await;
                if let Some(node) = node {
                    for seed in &peer_seeds.seeds {
                        if let Ok(seed_addr) = seed.address.parse::<std::net::SocketAddr>() {
                            node.registry.register(
                                seed.node_address.clone(),
                                seed_addr,
                                NetworkScope::from_ip(&seed_addr.ip()),
                            );
                            println!(
                                "  + Registered seed from peer: {} (node: {})",
                                seed.address, seed.node_address
                            );
                        }
                    }
                }
            } else {
                eprintln!("❌ Invalid seeds hash from peer!");
            }
        }

        // Generate seeds from NodeRegistry
        let node = ctx.lock().await.global.get::<Arc<P2pNode>>().await;
        let seed_records = if let Some(node) = node {
            let all_seeds: Vec<SeedRecord> = node
                .registry
                .get_all_seeds()
                .into_iter()
                .map(|(s, na)| SeedRecord::new(s.to_string(), na))
                .collect();
            SeedsCommand::new(all_seeds)
        } else {
            SeedsCommand::new(vec![])
        };

        println!(
            "📊 Consensus seeds: {} seeds, hash={:?}",
            seed_records.seeds.len(),
            seed_records.hash
        );
        Some(seed_records)
    };

    let ack = OnlineAckCommand {
        session_id: online.session_id,
        address: address.to_string(),
        node,
        ephemeral_public_key: ephemeral_public.to_bytes(),
        intranet_ips,
        wan_ips,
        seeds: seeds_to_send,
    };

    println!("send ack session_id : {:?}", ack.session_id);
    println!("send ack: {:?}", Codec::encode(&ack));

    P2PFrame::send::<OnlineAckCommand>(
        ctx.clone(),
        &Some(ack),
        Entity::Node,
        Action::OnLineAck,
        false,
    )
    .await
    .expect("Error send online ack!");
    println!("end of current online!");

    // Store the announced IPs from peer as external seeds
    for ip in online.intranet_ips.iter().chain(online.wan_ips.iter()) {
        if ip != "0.0.0.0" && !ip.starts_with("127.") {
            println!("Received external IP from peer: {}", ip);
        }
    }

    // 对称握手：作为 inbound 端，向对端发起出站连接（回连）
    // 但如果已有连接（任何方向），跳过以避免连接数膨胀
    let peer_sock = {
        let guard = ctx.lock().await;
        guard.addr
    };
    // Use the peer's advertised listening port from online.node,
    // NOT the ephemeral TCP source port (ctx.addr). This ensures
    // the return-connection hits the peer's actual listener.
    let peer_addr = std::net::SocketAddr::new(peer_sock.ip(), online.node.port);
    tracing::info!(
        "↩️ return-connect planning: peer_sock(inbound)={}, online.node.port={}, computed peer_addr={}",
        peer_sock,
        online.node.port,
        peer_addr
    );
    let gctx = {
        let guard = ctx.lock().await;
        guard.global.clone()
    };
    let scope = aex::connection::scope::NetworkScope::from_ip(&peer_addr.ip());
    // Check servers (outbound connections) only. The current inbound connection
    // should never block creating the return outbound; otherwise the mesh becomes
    // asymmetric (N0 gets 0 inbounds, other nodes get N-1). Each inbound must
    // produce exactly one outbound return-connect to balance the mesh.
    let already_connected = gctx
        .manager
        .connections
        .get(&(peer_addr.ip(), scope))
        .map(|bi_conn| {
            let in_servers = bi_conn.servers.contains_key(&peer_sock)
                || bi_conn.servers.contains_key(&peer_addr);
            let found = in_servers;
            let servers_addrs: Vec<std::net::SocketAddr> = bi_conn
                .servers
                .iter()
                .map(|ref_multi| *ref_multi.key())
                .collect();
            tracing::info!(
                "↩️ try_connect check: peer_sock={}, peer_addr={}, servers={:?}, found={}",
                peer_sock,
                peer_addr,
                servers_addrs,
                found,
            );
            found
        })
        .unwrap_or(false);

    if already_connected {
        tracing::info!(
            "↩️ Skip return connection to {} (already connected)",
            peer_addr
        );
    } else {
        let local_node = {
            let guard = gctx.local_node.write().await;
            guard.clone()
        };

        // Return connection: full handshake. The recipient's online_handler
        // will recognize this as is_return_conn=true (since the peer is already
        // marked connected) and skip key exchange. The session keys were already
        // established by the first (inbound) connection.
        let (session_id, _eph_pub) = psk.lock().await.create(false).await;
        let return_cmd = Arc::new(OnlineCommand {
            session_id: session_id.clone(),
            node: local_node,
            ephemeral_public_key: _eph_pub.to_bytes(),
            intranet_ips: vec![],
            wan_ips: vec![],
            seeds: None,
        });

        let cmd_clone = return_cmd.clone();
        let gctx_clone = gctx.clone();
        tokio::spawn(async move {
            match gctx_clone
                .manager
                .clone()
                .connect::<P2PFrame, P2PCommand, _, _>(
                    peer_addr,
                    gctx_clone.clone(),
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
                            )
                            .await
                            {
                                tracing::error!("❌ Failed to send return OnlineCommand: {:?}", e);
                                return;
                            }
                            if let Some(router) = aex::connection::context::get_tcp_router::<
                                P2PFrame,
                                P2PCommand,
                            >(&g.routers)
                            {
                                let _ = router.handle(new_ctx).await;
                            }
                        })
                    },
                    Some(10),
                )
                .await
            {
                Ok(_) => tracing::info!("✅ Return connection established to {}", peer_addr),
                Err(_) => tracing::info!("↩️ Return connection already exists to {}", peer_addr),
            }
        });
    }

    // Broadcast new peer to existing peers (mesh propagation)
    {
        let node = {
            let guard = ctx.lock().await;
            guard.global.get::<Arc<P2pNode>>().await
        };

        let seed_records = if let Some(node) = node {
            let all_seeds: Vec<SeedRecord> = node
                .registry
                .get_all_seeds()
                .into_iter()
                .map(|(s, na)| SeedRecord::new(s.to_string(), na))
                .collect();
            SeedsCommand::new(all_seeds)
        } else {
            SeedsCommand::new(vec![])
        };

        let ctx_for_broadcast = ctx.clone();
        tokio::spawn(async move {
            super::ack::broadcast_seeds_to_peers(ctx_for_broadcast, &seed_records).await;
        });
    }

    println!("end of online!");

    // 发布 peer online 事件，触发自动连接（事件驱动）
    {
        let ctx_guard = ctx.lock().await;
        let spread = &ctx_guard.global.spread;
        let event = PeerOnlineEvent {
            addr: frame.body.address.clone(),
            intranet_ips: online.intranet_ips.clone(),
            wan_ips: online.wan_ips.clone(),
        };
        let _ = spread.publish("peer_online", event).await;
    }

    // 双方握手后都向对端发起 node sync，确保双向同步
    let ctx_for_peer_sync = ctx.clone();
    let peer_addr = frame.body.address.clone();
    tokio::spawn(async move {
        // 延迟一点，让 ack 先完成
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        tracing::info!(
            "🔄 Triggering node sync with peer {} after online handshake...",
            peer_addr
        );
        if let Err(e) = crate::protocols::commands::node_sync::request_node_sync(
            ctx_for_peer_sync,
            peer_addr,
            "full".to_string(),
        )
        .await
        {
            eprintln!("❌ Failed to trigger node sync: {}", e);
        }
    });

    // 连接断开监控：当 TCP 连接关闭时自动清理 registry 中的 connected 标志
    let node_id_for_cleanup = frame.body.address.clone();
    let gctx_for_cleanup = {
        let guard = ctx.lock().await;
        guard.global.clone()
    };
    let peer_sock = {
        let guard = ctx.lock().await;
        guard.addr
    };
    let scope = aex::connection::scope::NetworkScope::from_ip(&peer_sock.ip());
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
            let still_connected = gctx_for_cleanup
                .manager
                .connections
                .get(&(peer_sock.ip(), scope))
                .map(|bi_conn| {
                    bi_conn.clients.contains_key(&peer_sock)
                        || bi_conn.servers.contains_key(&peer_sock)
                })
                .unwrap_or(false);
            if !still_connected {
                if let Some(node) = gctx_for_cleanup.get::<Arc<P2pNode>>().await {
                    node.registry.disconnect(&node_id_for_cleanup);
                    tracing::info!(
                        "🧹 Disconnected stale connection for node {}",
                        node_id_for_cleanup
                    );
                }
                let event = PeerOfflineEvent {
                    addr: node_id_for_cleanup.clone(),
                };
                let _ = gctx_for_cleanup.spread.publish("peer_offline", event).await;
                break;
            }
        }
    });
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerOnlineEvent {
    pub addr: String,
    pub intranet_ips: Vec<String>,
    pub wan_ips: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerOfflineEvent {
    pub addr: String,
}

pub fn get_all_ips() -> (Vec<String>, Vec<String>) {
    use std::net::Ipv4Addr;
    use std::process::Command;

    let mut intranet_ips = vec![];
    let mut wan_ips = vec![];

    // Virtual interface prefixes to filter (match any start)
    let virtual_prefixes = ["docker", "virbr", "veth", "br-", "lo"];

    if let Ok(output) = Command::new("ip").args(&["addr", "show"]).output() {
        if let Ok(output_str) = String::from_utf8(output.stdout) {
            let mut current_iface = "";
            for line in output_str.lines() {
                // Track interface name
                if !line.starts_with(char::is_whitespace) {
                    current_iface = line.split(':').nth(1).unwrap_or("").trim();
                }
                // Skip virtual interfaces
                if virtual_prefixes
                    .iter()
                    .any(|p| current_iface.starts_with(p))
                {
                    continue;
                }
                // Parse IPv4 addresses
                if line.trim().starts_with("inet ") {
                    let parts: Vec<&str> = line.trim().split_whitespace().collect();
                    if parts.len() >= 2 {
                        let ip_with_mask = parts[1];
                        let ip_str = ip_with_mask.split('/').next().unwrap_or("");
                        if let Ok(ip) = ip_str.parse::<Ipv4Addr>() {
                            if ip.is_loopback() || ip_str.starts_with("169.254") {
                                continue;
                            }
                            if ip.is_private() {
                                intranet_ips.push(ip_str.to_string());
                            } else {
                                wan_ips.push(ip_str.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    (intranet_ips, wan_ips)
}
