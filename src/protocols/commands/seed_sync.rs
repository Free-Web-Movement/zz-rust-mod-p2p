use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use aex::connection::context::Context;
use aex::connection::global::GlobalContext;
use aex::connection::node::Node;
use aex::tcp::types::Codec;

use crate::protocols::command::{Action, Entity, P2PCommand};
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
        let mut merged = self.seeds.clone();
        for seed in &other.seeds {
            if seed.ip.starts_with("127.") || seed.ip.starts_with("0.") || seed.ip.starts_with("::") {
                continue;
            }
            if !merged.iter().any(|s| s.ip == seed.ip && s.port == seed.port) {
                merged.push(seed.clone());
            }
        }
        SeedSet::with_pre_hash(merged, self.epoch.max(other.epoch), self.pre_hash.clone())
    }

    pub fn remove_unreachable(&self, unreachable: &[(String, u16)]) -> SeedSet {
        let filtered: Vec<SeedInfo> = self
            .seeds
            .iter()
            .filter(|s| !unreachable.contains(&(s.ip.clone(), s.port)))
            .cloned()
            .collect();
        SeedSet::with_pre_hash(filtered, self.epoch, self.pre_hash.clone())
    }

    pub fn filter_loopback(&self) -> SeedSet {
        let filtered: Vec<SeedInfo> = self
            .seeds
            .iter()
            .filter(|s| {
                !s.ip.starts_with("127.")
                    && !s.ip.starts_with("0.")
                    && !s.ip.starts_with("::1")
                    && !s.ip.starts_with("::")
            })
            .cloned()
            .collect();
        SeedSet::with_pre_hash(filtered, self.epoch, self.pre_hash.clone())
    }
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
    for s in &request.seed_set.seeds {
        println!("  → remote seed: {}:{}", s.ip, s.port);
    }

    // Merge incoming seeds from request into local seed set
    {
        let local_set = {
            let guard = ctx.lock().await;
            guard
                .global
                .get::<SeedSet>()
                .await
                .unwrap_or_else(|| SeedSet::new(vec![], 0))
        };
        println!("  → local seeds before merge: {}", local_set.len());
        for s in &local_set.seeds {
            println!("  → local seed: {}:{}", s.ip, s.port);
        }

        let merged = local_set.merge(&request.seed_set);
        println!("  → local seeds after merge: {}", merged.len());
        for s in &merged.seeds {
            println!("  → merged seed: {}:{}", s.ip, s.port);
        }

        let guard = ctx.lock().await;
        guard.global.set(merged).await;
        drop(guard);
    }

    let response = {
        let guard = ctx.lock().await;
        let seed_set = guard
            .global
            .get::<SeedSet>()
            .await
            .unwrap_or_else(|| SeedSet::new(vec![], 0));

        let filtered_set = seed_set.filter_loopback();
        println!("  → response seeds (after filter_loopback): {}", filtered_set.len());
        for s in &filtered_set.seeds {
            println!("  → response seed: {}:{}", s.ip, s.port);
        }

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
    for s in &response.seed_set.seeds {
        println!("  ← response seed: {}:{}", s.ip, s.port);
    }

    let local_set = {
        let guard = ctx.lock().await;
        guard
            .global
            .get::<SeedSet>()
            .await
            .unwrap_or_else(|| SeedSet::new(vec![], 0))
    };
    println!("  ← local seeds before merge: {}", local_set.len());
    for s in &local_set.seeds {
        println!("  ← local seed: {}:{}", s.ip, s.port);
    }

    let merged = local_set.merge(&response.seed_set);
    println!("  ← local seeds after merge: {}", merged.len());
    for s in &merged.seeds {
        println!("  ← merged seed: {}:{}", s.ip, s.port);
    }

    let guard = ctx.lock().await;
    guard.global.set(merged).await;
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
    guard.global.set(commit.seed_set.clone()).await;
    drop(guard);
}

pub fn register_seed_sync_handlers(_cli: &mut crate::cli::Cli) {
    // Handlers are registered via the protocol router
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
    max_retries: u32,
    tick_deadline: Instant,
) -> Result<SeedSet, String> {
    let mut current_set = gctx
        .get::<SeedSet>()
        .await
        .unwrap_or_else(|| SeedSet::new(vec![], 0));

    for round in 1..=max_retries {
        if Instant::now() >= tick_deadline {
            return Err("Sync timeout: exceeded tick deadline".into());
        }

        println!("--- Seed sync round {}/{} ---", round, max_retries);

        for addr in &peer_addrs {
            if Instant::now() >= tick_deadline {
                break;
            }

            let success = try_sync_with_peer_global(gctx.clone(), *addr).await;
            update_seed_status_global(&mut current_set, addr, success);
        }

        let new_set = gctx
            .get::<SeedSet>()
            .await
            .unwrap_or_else(|| current_set.clone());

        if new_set.verify() && !new_set.is_empty() {
            println!("✅ Seed sync converged after {} rounds", round);
            return Ok(new_set);
        }

        current_set = new_set;
        current_set.sync_round = round;
    }

    println!(
        "⚠️ Seed sync did not converge after {} rounds, pruning unreachable",
        max_retries
    );

    let unreachable = find_unreachable_seeds(&current_set);
    let pruned = current_set.remove_unreachable(&unreachable);

    gctx.set(pruned.clone()).await;

    if Instant::now() < tick_deadline {
        for addr in &peer_addrs {
            if Instant::now() >= tick_deadline {
                break;
            }
            let _success = try_sync_with_peer_global(gctx.clone(), *addr).await;
        }
    }

    Ok(pruned)
}

async fn try_sync_with_peer_global(gctx: Arc<GlobalContext>, addr: SocketAddr) -> bool {
    use aex::connection::context::get_tcp_router;

    let result = Arc::new(AtomicBool::new(false));
    let result_clone = result.clone();

    let psk = gctx.paired_session_keys.clone().unwrap();

    tracing::info!("🔗 Attempting to connect to peer: {}", addr);

    match gctx.manager.clone().connect::<P2PFrame, P2PCommand, _, _>(
        addr,
        gctx.clone(),
        move |ctx| {
            let result = result_clone.clone();
            let gctx = gctx.clone();
            let psk = psk.clone();
            Box::pin(async move {
                tracing::info!("✅ Connected to peer: {}, sending seed sync request", addr);

                let (id, _key) = {
                    let guard = psk.lock().await;
                    guard.create(false).await
                };

                let _aex_node = Node::from_system(addr.port(), id.clone(), 1);

                let local_set = gctx
                    .get::<SeedSet>()
                    .await
                    .unwrap_or_else(|| SeedSet::new(vec![], 0));

                tracing::info!("📤 Sending {} seeds to peer {}", local_set.len(), addr);

                let request = SeedSyncRequest {
                    from_node_id: gctx.addr.to_string(),
                    seed_set: local_set.clone(),
                    retry_count: local_set.sync_round,
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
                    tracing::info!("✅ Request sent to {}, starting response reader...", addr);

                    // Start reading responses from the peer
                    if let Some(router) = get_tcp_router::<P2PFrame, P2PCommand>(&gctx.routers) {
                        let _ = router.handle(ctx.clone()).await;
                    }

                    tokio::time::sleep(Duration::from_millis(500)).await;

                    let merged = gctx
                        .get::<SeedSet>()
                        .await
                        .unwrap_or_else(|| local_set.clone());

                    tracing::info!("📊 After sync with {}: {} seeds", addr, merged.len());
                    for s in &merged.seeds {
                        tracing::info!("  → seed: {}:{}", s.ip, s.port);
                    }

                    if merged.verify() {
                        result.store(true, Ordering::SeqCst);
                    }
                } else {
                    tracing::error!("❌ Failed to send request to {}: {:?}", addr, send_result);
                }
            })
        },
        Some(5),
    )
    .await
    {
        Ok(_) => {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let success = result.load(Ordering::SeqCst);
            tracing::info!("🏁 Sync with {} result: {}", addr, if success { "SUCCESS" } else { "FAILED" });
            success
        }
        Err(e) => {
            tracing::error!("❌ Connection to {} failed: {}", addr, e);
            false
        }
    }
}

fn update_seed_status_global(seed_set: &mut SeedSet, addr: &SocketAddr, success: bool) {
    let target_ip = addr.ip().to_string();
    let target_port = addr.port();
    for seed in &mut seed_set.seeds {
        if seed.ip == target_ip && seed.port == target_port {
            if success {
                seed.success_count += 1;
            } else {
                seed.failure_count += 1;
            }
            seed.last_attempt_ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            return;
        }
    }
}

fn find_unreachable_seeds(seed_set: &SeedSet) -> Vec<(String, u16)> {
    seed_set
        .seeds
        .iter()
        .filter(|s| s.failure_count >= 3 || s.failure_count > s.success_count * 2)
        .map(|s| (s.ip.clone(), s.port))
        .collect()
}
