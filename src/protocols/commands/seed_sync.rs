use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use aex::connection::context::Context;
use aex::connection::global::GlobalContext;
use aex::connection::scope::NetworkScope;
use aex::tcp::types::Codec;

use crate::node::Node;
use crate::protocols::command::{Action, Entity, P2PCommand};
use crate::protocols::commands::node_registry::NodeRegistry;
use crate::protocols::frame::P2PFrame;

pub const SEED_SYNC_MAX_RETRIES: u32 = 3;
pub const SEED_HASH_HEX_LENGTH: usize = 64;

pub fn default_pre_hash() -> String {
    "0".repeat(SEED_HASH_HEX_LENGTH)
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct SeedInfo {
    pub ip: String,
    pub port: u16,
    pub node_id: String,
    pub is_intranet: bool,
    pub is_connectable: bool,
    pub success_count: u32,
    pub failure_count: u32,
    pub last_attempt_ts: u64,
}

impl SeedInfo {
    pub fn new(ip: String, port: u16, node_id: String, is_intranet: bool) -> Self {
        Self {
            ip,
            port,
            node_id,
            is_intranet,
            is_connectable: true,
            success_count: 0,
            failure_count: 0,
            last_attempt_ts: 0,
        }
    }

    pub fn address(&self) -> String {
        format!("{}:{}", self.ip, self.port)
    }

    pub fn socket_addr(&self) -> Option<SocketAddr> {
        format!("{}:{}", self.ip, self.port).parse().ok()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct SeedSet {
    pub seeds: Vec<SeedInfo>,
    pub pre_hash: String,
    pub hash: [u8; 32],
    pub epoch: u64,
    pub locked: bool,
    pub sync_round: u32,
}

impl SeedSet {
    pub fn new(seeds: Vec<SeedInfo>, epoch: u64) -> Self {
        let pre_hash = default_pre_hash();
        let hash = compute_seed_hash(&pre_hash, &seeds);
        Self { seeds, pre_hash, hash, epoch, locked: false, sync_round: 0 }
    }

    pub fn with_pre_hash(seeds: Vec<SeedInfo>, epoch: u64, pre_hash: String) -> Self {
        let hash = compute_seed_hash(&pre_hash, &seeds);
        Self { seeds, pre_hash, hash, epoch, locked: false, sync_round: 0 }
    }

    pub fn verify(&self) -> bool {
        let computed = compute_seed_hash(&self.pre_hash, &self.seeds);
        computed == self.hash
    }

    pub fn recalculate_hash(&mut self) {
        self.hash = compute_seed_hash(&self.pre_hash, &self.seeds);
    }

    pub fn is_empty(&self) -> bool {
        self.seeds.is_empty()
    }

    pub fn len(&self) -> usize {
        self.seeds.len()
    }

    pub fn contains(&self, ip: &str, port: u16) -> bool {
        self.seeds.iter().any(|s| s.ip == ip && s.port == port)
    }

    pub fn merge(&self, other: &SeedSet) -> SeedSet {
        let mut merged: Vec<SeedInfo> = self.seeds.iter()
            .filter(|s| !Self::is_loopback(&s.ip))
            .cloned()
            .collect();
        let mut seen: std::collections::HashSet<String> = merged.iter().map(|s| s.address()).collect();
        
        for seed in &other.seeds {
            if Self::is_loopback(&seed.ip) {
                continue;
            }
            let addr = seed.address();
            if !seen.contains(&addr) {
                seen.insert(addr);
                merged.push(seed.clone());
            }
        }
        SeedSet::with_pre_hash(merged, self.epoch.max(other.epoch), self.pre_hash.clone())
    }

    fn is_loopback(ip: &str) -> bool {
        if let Ok(addr) = ip.parse::<std::net::IpAddr>() {
            addr.is_loopback()
        } else {
            false
        }
    }

    pub fn filter_loopback(&self) -> SeedSet {
        let filtered: Vec<SeedInfo> = self
            .seeds
            .iter()
            .filter(|s| !Self::is_loopback(&s.ip))
            .cloned()
            .collect();
        SeedSet::with_pre_hash(filtered, self.epoch, self.pre_hash.clone())
    }
}

pub fn derive_seed_set_from_registry(reg: &NodeRegistry) -> SeedSet {
    let seeds: Vec<SeedInfo> = reg
        .get_all_seeds()
        .into_iter()
        .filter(|(addr, _)| addr.port() != 0)
        .map(|(addr, node_addr)| {
            let is_intranet =
                matches!(addr.ip(), std::net::IpAddr::V4(v4) if v4.is_private());
            SeedInfo::new(addr.ip().to_string(), addr.port(), node_addr, is_intranet)
        })
        .collect();
    SeedSet::new(seeds, 0)
}

pub fn compute_seed_hash(pre_hash: &str, seeds: &[SeedInfo]) -> [u8; 32] {
    let mut sorted: Vec<String> = seeds.iter().map(|s| s.address()).collect();
    sorted.sort();
    
    let mut hasher = Sha256::default();
    hasher.update(pre_hash.as_bytes());
    for addr in &sorted {
        hasher.update(addr.as_bytes());
    }
    hasher.finalize().into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct SeedSyncRequest {
    pub from_node_id: String,
    pub seed_set: SeedSet,
    pub retry_count: u32,
}

impl Codec for SeedSyncRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct SeedSyncResponse {
    pub seed_set: SeedSet,
    pub hash: [u8; 32],
}

impl Codec for SeedSyncResponse {}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct SeedSyncCommit {
    pub hash: [u8; 32],
    pub seed_set: SeedSet,
    pub locked: bool,
}

impl Codec for SeedSyncCommit {}

pub async fn seed_sync_request_handler(
    ctx: Arc<Mutex<Context>>,
    _frame: P2PFrame,
    cmd: P2PCommand,
) {
    let request: SeedSyncRequest = match Codec::decode(&cmd.data) {
        Ok(req) => req,
        Err(e) => {
            eprintln!("❌ decode SeedSyncRequest failed: {e}");
            return;
        }
    };

    println!(
        "🔄 Seed sync request from {} (retry={}), their seeds: {}",
        request.from_node_id, request.retry_count, request.seed_set.len()
    );

    let guard = ctx.lock().await;
    if let Some(node) = guard.global.get::<Arc<Node>>().await {
        for seed in &request.seed_set.seeds {
            if let Some(seed_addr) = seed.socket_addr() {
                let scope = NetworkScope::from_ip(&seed_addr.ip());
                node.registry.register(seed.node_id.clone(), seed_addr, scope);
            }
        }

        let mut peer_guard = node.peer_addrs.write().await;
        let seed_set = derive_seed_set_from_registry(&node.registry);
        for seed in &seed_set.seeds {
            if let Some(seed_addr) = seed.socket_addr() {
                if !peer_guard.contains(&seed_addr) {
                    peer_guard.push(seed_addr);
                }
            }
        }
    }
    drop(guard);

    let response = {
        let guard = ctx.lock().await;
        let node = guard
            .global
            .get::<Arc<Node>>()
            .await
            .unwrap();

        let seed_set = derive_seed_set_from_registry(&node.registry);
        let filtered_set = seed_set.filter_loopback();
        println!("  → response seeds (after filter_loopback): {}", filtered_set.len());

        SeedSyncResponse {
            hash: filtered_set.hash,
            seed_set: filtered_set,
        }
    };

    let _ = P2PFrame::send::<SeedSyncResponse>(
        ctx.clone(),
        &Some(response),
        Entity::Node,
        Action::SeedSyncResponse,
        false,
    )
    .await;
}

pub async fn seed_sync_response_handler(
    ctx: Arc<Mutex<Context>>,
    _frame: P2PFrame,
    cmd: P2PCommand,
) {
    let response: SeedSyncResponse = match Codec::decode(&cmd.data) {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("❌ decode SeedSyncResponse failed: {e}");
            return;
        }
    };

    println!(
        "📥 Seed sync response: {} seeds, hash={:?}",
        response.seed_set.len(),
        &response.hash[..8]
    );

    let guard = ctx.lock().await;
    if let Some(node) = guard.global.get::<Arc<Node>>().await {
        for seed in &response.seed_set.seeds {
            if let Some(seed_addr) = seed.socket_addr() {
                let scope = NetworkScope::from_ip(&seed_addr.ip());
                node.registry.register(seed.node_id.clone(), seed_addr, scope);
            }
        }

        let mut peer_guard = node.peer_addrs.write().await;
        let seed_set = derive_seed_set_from_registry(&node.registry);
        for seed in &seed_set.seeds {
            if let Some(seed_addr) = seed.socket_addr() {
                if !peer_guard.contains(&seed_addr) {
                    peer_guard.push(seed_addr);
                }
            }
        }
    }
    drop(guard);
}

pub async fn seed_sync_commit_handler(
    ctx: Arc<Mutex<Context>>,
    _frame: P2PFrame,
    cmd: P2PCommand,
) {
    let commit: SeedSyncCommit = match Codec::decode(&cmd.data) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("❌ decode SeedSyncCommit failed: {e}");
            return;
        }
    };

    println!(
        "🔒 Seed sync committed: hash={:?}, locked={}",
        &commit.hash[..8],
        commit.locked
    );

    let guard = ctx.lock().await;
    if let Some(node) = guard.global.get::<Arc<Node>>().await {
        for seed in &commit.seed_set.seeds {
            if let Some(seed_addr) = seed.socket_addr() {
                let scope = NetworkScope::from_ip(&seed_addr.ip());
                node.registry.register(seed.node_id.clone(), seed_addr, scope);
            }
        }

        let mut peer_guard = node.peer_addrs.write().await;
        let seed_set = derive_seed_set_from_registry(&node.registry);
        for seed in &seed_set.seeds {
            if let Some(seed_addr) = seed.socket_addr() {
                if !peer_guard.contains(&seed_addr) {
                    peer_guard.push(seed_addr);
                }
            }
        }
    }
    drop(guard);
}

pub fn register_seed_sync_handlers(_cli: &mut crate::cli::Cli) {
    // Handlers are registered via the protocol router
}

pub async fn broadcast_seed_set_to_all_peers(ctx: Arc<Mutex<Context>>, seed_set: &SeedSet) {
    let gctx = {
        let guard = ctx.lock().await;
        guard.global.clone()
    };

    let address = gctx.get::<zz_account::address::FreeWebMovementAddress>().await;
    let address = match address {
        Some(addr) => addr,
        None => return,
    };

    let request = SeedSyncRequest {
        from_node_id: gctx.addr.to_string(),
        seed_set: seed_set.clone(),
        retry_count: seed_set.sync_round,
    };

    let cmd_bytes = Codec::encode(&request);
    let p2p_cmd = P2PCommand::new(Entity::Node, Action::SeedSyncRequest, cmd_bytes);
    let frame = P2PFrame::build(&address, p2p_cmd, 1).await.unwrap();
    let frame_bytes = Codec::encode(&frame);
    let seed_count = seed_set.len();

    let manager = gctx.manager.clone();

    manager
        .forward(|entries| async move {
            for entry in entries {
                if let Some(peer_ctx) = &entry.context {
                    let mut guard = peer_ctx.lock().await;
                    if let Some(writer) = &mut guard.writer {
                        if let Err(e) = writer.write_all(&frame_bytes).await {
                            eprintln!("  ❌ Failed to broadcast seed set: {:?}", e);
                        } else {
                            let _ = writer.flush().await;
                        }
                    }
                }
            }
        })
        .await;

    println!("  📢 Broadcast seed set ({} seeds) to all peers", seed_count);
}

pub async fn lock_seed_set_global(gctx: Arc<GlobalContext>, mut seed_set: SeedSet) {
    seed_set.locked = true;
    gctx.set(seed_set.clone()).await;

    println!(
        "🔒 Seed set locked: {} seeds, hash={:?}",
        seed_set.len(),
        &seed_set.hash[..8]
    );
}

pub async fn run_seed_sync_cycle(
    gctx: Arc<GlobalContext>,
    peer_addrs: Vec<SocketAddr>,
    _max_retries: u32,
    tick_deadline: Instant,
) -> Result<SeedSet, String> {
    if peer_addrs.is_empty() {
        let seed_set = match gctx.get::<Arc<Node>>().await {
            Some(node) => derive_seed_set_from_registry(&node.registry),
            None => SeedSet::new(vec![], 0),
        };
        return Ok(seed_set);
    }

    const STABLE_ROUNDS_REQUIRED: u32 = 2;
    const MAX_ROUNDS: u32 = 10;
    let mut stable_rounds = 0;
    let mut prev_hash: Option<[u8; 32]> = None;
    let mut round = 0u32;

    while Instant::now() < tick_deadline && round < MAX_ROUNDS {
        round += 1;

        let current_seed_set = {
            let node = match gctx.get::<Arc<Node>>().await {
                Some(node) => node,
                None => return Err("Node not registered in GlobalContext".to_string()),
            };
            derive_seed_set_from_registry(&node.registry)
        };

        tracing::info!("=== Seed sync round {}/{} with {} peers ({} seeds), stable_rounds={} ===", round, MAX_ROUNDS, peer_addrs.len(), current_seed_set.len(), stable_rounds);

        for addr in &peer_addrs {
            if Instant::now() >= tick_deadline {
                break;
            }
            let _ = try_sync_with_peer_global(gctx.clone(), *addr).await;
        }

        tokio::time::sleep(Duration::from_secs(2)).await;

        let current_hash = {
            let node = match gctx.get::<Arc<Node>>().await {
                Some(node) => node,
                None => return Err("Node not registered in GlobalContext".to_string()),
            };
            let seed_set = derive_seed_set_from_registry(&node.registry);
            seed_set.hash
        };

        if prev_hash == Some(current_hash) {
            stable_rounds += 1;
            tracing::info!("📊 SeedSet hash unchanged, stable_rounds={}/{}", stable_rounds, STABLE_ROUNDS_REQUIRED);
            if stable_rounds >= STABLE_ROUNDS_REQUIRED {
                let node = gctx.get::<Arc<Node>>().await.unwrap();
                let current_set = derive_seed_set_from_registry(&node.registry);
                tracing::info!("✅ Seed sync converged after {} rounds with {} seeds, hash={:?}", round, current_set.len(), &current_hash[..8]);
                return Ok(current_set);
            }
        } else {
            stable_rounds = 0;
            prev_hash = Some(current_hash);
            let node = gctx.get::<Arc<Node>>().await.unwrap();
            let set_len = derive_seed_set_from_registry(&node.registry).len();
            tracing::info!("📊 SeedSet hash changed, {} seeds, hash={:?}", set_len, &current_hash[..8]);
        }
    }

    tracing::warn!("⏰ Seed sync timeout after {} rounds", round);
    let seed_set = match gctx.get::<Arc<Node>>().await {
        Some(node) => derive_seed_set_from_registry(&node.registry),
        None => SeedSet::new(vec![], 0),
    };
    Ok(seed_set)
}

async fn try_sync_with_peer_global(gctx: Arc<GlobalContext>, addr: SocketAddr) -> bool {
    // Try up to 3 times per peer per round
    for attempt in 1..=3 {
        let result = sync_once(gctx.clone(), addr, attempt).await;
        if result {
            return true;
        }
        if attempt < 3 {
            tokio::time::sleep(Duration::from_millis(500 * attempt as u64)).await;
        }
    }
    false
}

async fn sync_once(gctx: Arc<GlobalContext>, addr: SocketAddr, attempt: u32) -> bool {
    let result = Arc::new(AtomicBool::new(false));
    let result_clone = result.clone();

    let psk = gctx.paired_session_keys.clone().unwrap();

    // First try to find existing connection (inbound or outbound)
    if let Some(existing_ctx) = get_existing_connection_context(&gctx, addr) {
        tracing::info!("🔗 Using existing connection to peer: {} (attempt {})", addr, attempt);
        send_seed_sync_via_existing_ctx(existing_ctx, addr, psk, gctx.clone(), result_clone, attempt).await;
        tokio::time::sleep(Duration::from_millis(1000)).await;
        let success = result.load(Ordering::SeqCst);
        tracing::info!("🏁 Sync with {} (existing) attempt {} result: {}", addr, attempt, if success { "SUCCESS" } else { "FAILED" });
        return success;
    }

    // No existing connection, try to connect
    tracing::info!("🔗 Attempting to connect to peer: {} (attempt {})", addr, attempt);

    match gctx.manager.clone().connect::<P2PFrame, P2PCommand, _, _>(
        addr,
        gctx.clone(),
        move |ctx| {
            let result = result_clone.clone();
            let gctx = gctx.clone();
            let psk = psk.clone();
            Box::pin(async move {
                tracing::info!("✅ Connected to peer: {}, sending seed sync request (attempt {})", addr, attempt);
                send_seed_sync_request(ctx, addr, psk, gctx, result, attempt).await;
            })
        },
        Some(5),
    )
    .await
    {
        Ok(_) => {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            let success = result.load(Ordering::SeqCst);
            tracing::info!("🏁 Sync with {} (new) attempt {} result: {}", addr, attempt, if success { "SUCCESS" } else { "FAILED" });
            success
        }
        Err(e) => {
            tracing::error!("❌ Connection to {} attempt {} failed: {}", addr, attempt, e);
            false
        }
    }
}

fn get_existing_connection_context(gctx: &GlobalContext, addr: SocketAddr) -> Option<Arc<Mutex<aex::connection::context::Context>>> {
    let manager = &gctx.manager;
    let ip = addr.ip();
    let scope = aex::connection::scope::NetworkScope::from_ip(&ip);

    if let Some(bi_conn) = manager.connections.get(&(ip, scope)) {
        // Try servers (outbound) first, then clients (inbound)
        if let Some(entry) = bi_conn.servers.get(&addr) {
            return entry.value().context.clone();
        }
        if let Some(entry) = bi_conn.clients.get(&addr) {
            return entry.value().context.clone();
        }
    }
    None
}

async fn send_seed_sync_via_existing_ctx(
    ctx: Arc<Mutex<aex::connection::context::Context>>,
    addr: SocketAddr,
    psk: Arc<tokio::sync::Mutex<aex::crypto::session_key_manager::PairedSessionKey>>,
    gctx: Arc<GlobalContext>,
    result: Arc<AtomicBool>,
    attempt: u32,
) {
    let _ = psk.lock().await;

    let local_set = match gctx.get::<Arc<Node>>().await {
        Some(node) => derive_seed_set_from_registry(&node.registry),
        None => SeedSet::new(vec![], 0),
    };

    tracing::info!("📤 Sending {} seeds to peer {} (existing conn, attempt {})", local_set.len(), addr, attempt);

    let request = SeedSyncRequest {
        from_node_id: gctx.addr.to_string(),
        seed_set: local_set.clone(),
        retry_count: 0,
    };

    match P2PFrame::send::<SeedSyncRequest>(
        ctx.clone(),
        &Some(request),
        Entity::Node,
        Action::SeedSyncRequest,
        false,
    )
    .await {
        Ok(_) => {
            tracing::info!("✅ Request sent to {}, waiting for response...", addr);
            tokio::time::sleep(Duration::from_millis(2000)).await;

            let merged = match gctx.get::<Arc<Node>>().await {
                Some(node) => derive_seed_set_from_registry(&node.registry),
                None => local_set.clone(),
            };

            tracing::info!("📊 After sync with {}: {} seeds (was {})", addr, merged.len(), local_set.len());

            if merged.verify() {
                result.store(true, Ordering::SeqCst);
                tracing::info!("✅ Sync successful: valid SeedSet with {} seeds", merged.len());
            } else {
                tracing::warn!("❌ Sync failed: SeedSet verification failed");
            }
        }
        Err(e) => {
            tracing::error!("❌ Failed to send request to {}: {:?}", addr, e);
        }
    }
}

async fn send_seed_sync_request(
    ctx: Arc<Mutex<aex::connection::context::Context>>,
    addr: SocketAddr,
    psk: Arc<tokio::sync::Mutex<aex::crypto::session_key_manager::PairedSessionKey>>,
    gctx: Arc<GlobalContext>,
    result: Arc<AtomicBool>,
    attempt: u32,
) {
    use aex::connection::context::get_tcp_router;

    let (_id, _key) = {
        let guard = psk.lock().await;
        guard.create(false).await
    };

    let local_set = match gctx.get::<Arc<Node>>().await {
        Some(node) => derive_seed_set_from_registry(&node.registry),
        None => SeedSet::new(vec![], 0),
    };

    tracing::info!("📤 Sending {} seeds to peer {} (attempt {})", local_set.len(), addr, attempt);

    let request = SeedSyncRequest {
        from_node_id: gctx.addr.to_string(),
        seed_set: local_set.clone(),
        retry_count: 0,
    };

    let send_result = P2PFrame::send::<SeedSyncRequest>(
        ctx.clone(),
        &Some(request),
        Entity::Node,
        Action::SeedSyncRequest,
        false,
    )
    .await;

    if send_result.is_ok() {
        tracing::info!("✅ Request sent to {}, waiting for response...", addr);

        let ctx_clone = ctx.clone();
        let gctx_clone = gctx.clone();
        tokio::spawn(async move {
            if let Some(router) = get_tcp_router::<P2PFrame, P2PCommand>(&gctx_clone.routers) {
                let _ = router.handle(ctx_clone).await;
            }
        });

        tokio::time::sleep(Duration::from_millis(2000)).await;

        let merged = match gctx.get::<Arc<Node>>().await {
            Some(node) => derive_seed_set_from_registry(&node.registry),
            None => local_set.clone(),
        };

        tracing::info!("📊 After sync with {}: {} seeds (was {})", addr, merged.len(), local_set.len());
        for s in &merged.seeds {
            tracing::info!("  → seed: {}:{}", s.ip, s.port);
        }

        if merged.verify() {
            result.store(true, Ordering::SeqCst);
        }
    } else {
        tracing::error!("❌ Failed to send request to {}: {:?}", addr, send_result);
    }
}


