use std::sync::Arc;

pub use crate::web::aex_re_exports::{
    ConnectionInfo, Context, GlobalContext, HeaderKey, HttpMetadata, NetworkScope, PeerInfo,
    SubMediaType,
};
use askama::Template;
use base64::Engine;
use chrono::Utc;
use sea_orm::DatabaseConnection;
use crate::node::Node;
use crate::protocols::commands::node_registry::NodeRegistry;
use crate::protocols::commands::node_sync::SeedData;

use crate::db::defines::StoreFromConnection;
use crate::user_store::UserStore;

use super::templates::{
    self, AccountInfo, ChatTemplate, NetworkTemplate, ResourceInfo, TransactionInfo,
    TransactionPage, WalletTemplate, WitnessRingInfo, WitnessTableInfo,
};

const DEFAULT_EXPLORER_URL: &str = "https://freeweb.observer/explorer";

// ===================== Helper functions =====================

pub async fn read_http_body(ctx: &mut Context) -> (usize, Vec<u8>) {
    use tokio::io::AsyncReadExt;
    let cl = ctx
        .local
        .get_ref::<HttpMetadata>()
        .and_then(|m| m.headers.get(&HeaderKey::ContentLength))
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; cl.max(4096)];
    if let Some(reader) = ctx.reader.as_deref_mut() {
        let _ = reader.read_exact(&mut body[..cl]).await;
    }
    (cl, body)
}

fn get_query_param<'a>(path: &'a str, key: &str) -> Option<&'a str> {
    let query = path.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if parts.next()? == key {
            return parts.next();
        }
    }
    None
}

fn url_decode_query(val: &str) -> String {
    let mut result = Vec::with_capacity(val.len());
    let mut bytes = val.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            let hi = bytes.next().and_then(hex_val);
            let lo = bytes.next().and_then(hex_val);
            if let (Some(h), Some(l)) = (hi, lo) {
                result.push(h << 4 | l);
            }
        } else if b == b'+' {
            result.push(b' ');
        } else {
            result.push(b);
        }
    }
    String::from_utf8(result).unwrap_or_else(|_| val.to_string())
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn is_valid_seed_ip(ip: &str) -> bool {
    !ip.starts_with("127.") && ip != "0.0.0.0" && ip != "::1" && ip != "::"
}

fn seed_type_from_ip(ip: &str) -> &'static str {
    if let Ok(addr) = ip.parse::<std::net::IpAddr>() {
        match addr {
            std::net::IpAddr::V6(_) => "ipv6",
            std::net::IpAddr::V4(v4) => {
                if v4.is_private() || v4.is_loopback() || v4.is_link_local() {
                    "inner"
                } else {
                    "external"
                }
            }
        }
    } else {
        "external"
    }
}

pub async fn read_seeds_from_db(db: &DatabaseConnection) -> Vec<SeedData> {
    use crate::db::entity::witness_seed::store::WitnessSeedStore;
    let store = WitnessSeedStore::new(db);
    match store.get_active_seeds().await {
        Ok(seeds) => seeds
            .into_iter()
            .map(|m| SeedData {
                id: m.id,
                ip: m.ip.clone(),
                port: m.port as i32,
                node_id: m.node_id,
                is_intranet: m.is_intranet,
                is_active: m.is_active,
                success_count: m.success_count,
                failure_count: m.failure_count,
                online_days: m.online_days,
                offline_days: m.offline_days,
                online_rate: m.online_rate,
            })
            .collect(),
        Err(e) => {
            tracing::error!("Failed to read seeds from DB: {}", e);
            vec![]
        }
    }
}

pub async fn write_seeds_to_db(db: &DatabaseConnection, seeds: &[SeedData]) {
    use crate::db::entity::witness_seed::store::WitnessSeedStore;
    let store = WitnessSeedStore::new(db);
    if let Err(e) = store.clear_all().await {
        tracing::warn!("Failed to clear old seeds: {}", e);
    }
    for seed in seeds {
        let (ip, port) = if let Some(pos) = seed.ip.rfind(':') {
            let (ip_part, port_part) = seed.ip.split_at(pos);
            let port_num = port_part[1..].parse::<i32>().unwrap_or(seed.port);
            (ip_part.to_string(), port_num)
        } else {
            (seed.ip.clone(), seed.port)
        };
        if let Err(e) = store
            .upsert(&ip, port, &seed.node_id, seed.is_intranet)
            .await
        {
            tracing::error!("Failed to write seed {}:{} to DB: {}", ip, port, e);
        }
    }
    tracing::info!("✅ Synced {} seeds to database (full replace)", seeds.len());
}

async fn load_seeds_from_db(db: &DatabaseConnection) -> Vec<templates::SeedInfo> {
    use crate::db::entity::witness_seed::store::WitnessSeedStore;
    let store = WitnessSeedStore::new(db);
    match store.get_active_seeds().await {
        Ok(seeds) => seeds
            .into_iter()
            .filter(|m| is_valid_seed_ip(&m.ip))
            .map(|m| templates::SeedInfo {
                address: format!("{}:{}", m.ip, m.port),
                port: m.port as u16,
                protocol: seed_type_from_ip(&m.ip).to_string(),
                is_active: m.is_active,
                last_seen: chrono::DateTime::from_timestamp(m.last_seen, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
            })
            .collect(),
        Err(e) => {
            tracing::error!("Failed to load seeds from DB for web: {}", e);
            vec![]
        }
    }
}

async fn load_seeds_from_global_context(gctx: Arc<GlobalContext>) -> Vec<templates::SeedInfo> {
    use crate::protocols::commands::seed_sync::derive_seed_set_from_registry;
    let node = match gctx.get::<Arc<Node>>().await {
        Some(n) => n,
        None => return vec![],
    };
    let seed_set = derive_seed_set_from_registry(&node.registry);
    seed_set
        .seeds
        .iter()
        .filter(|s| is_valid_seed_ip(&s.ip))
        .map(|s| templates::SeedInfo {
            address: format!("{}:{}", s.ip, s.port),
            port: s.port,
            protocol: seed_type_from_ip(&s.ip).to_string(),
            is_active: true,
            last_seen: "now".to_string(),
        })
        .collect()
}

async fn load_all_seeds(
    db: &DatabaseConnection,
    gctx: Arc<GlobalContext>,
) -> Vec<templates::SeedInfo> {
    let mut seeds = load_seeds_from_db(db).await;
    let ctx_seeds = load_seeds_from_global_context(gctx).await;
    let mut seen: std::collections::HashSet<String> =
        seeds.iter().map(|s| s.address.clone()).collect();
    for seed in ctx_seeds {
        if !seen.contains(&seed.address) {
            seen.insert(seed.address.clone());
            seeds.push(seed);
        }
    }
    seeds
}

pub async fn load_transactions(
    db: &DatabaseConnection,
    page: u64,
    per_page: u64,
) -> TransactionPage {
    use crate::db::entity::transaction::store::TransactionStore;
    let store = TransactionStore::new(db);
    let total_count = store.count_all().await.unwrap_or(0);
    let total_pages = if total_count == 0 {
        1
    } else {
        (total_count + per_page - 1) / per_page
    };
    let transactions: Vec<TransactionInfo> = match store.get_page(page, per_page).await {
        Ok(rows) => rows
            .into_iter()
            .map(|tx| {
                let amount_i64: i64 = tx.amount.parse().unwrap_or(0);
                TransactionInfo {
                    id: tx.id,
                    tx_type: format!("{:?}", tx.tx_type),
                    from_address: tx.from_address,
                    to_address: tx.to_address,
                    amount: format!(
                        "{}.{:>02}",
                        amount_i64 / 100,
                        (amount_i64 % 100).abs() as u8
                    ),
                    timestamp: chrono::DateTime::from_timestamp(tx.timestamp, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_default(),
                    hash: tx.hash,
                    pre_hash: tx.pre_hash,
                }
            })
            .collect(),
        Err(_) => vec![],
    };
    TransactionPage {
        transactions,
        current_page: page,
        total_pages: total_pages.max(1),
        total_count,
    }
}

pub fn group_connections_by_direction(
    raw: ConnectionInfo,
    reg: &NodeRegistry,
    local_node_id: &str,
) -> templates::ConnectionsByDirection {
    use std::collections::{HashMap, HashSet};
    use std::net::{IpAddr, SocketAddr};
    let reg_nodes = reg.get_nodes();
    let mut by_seed: HashMap<SocketAddr, &str> = HashMap::new();
    let mut by_ip: HashMap<IpAddr, Vec<&str>> = HashMap::new();
    for entry in &reg_nodes {
        for (seed, _) in &entry.seeds {
            by_seed.insert(*seed, &entry.address);
            by_ip.entry(seed.ip()).or_default().push(&entry.address);
        }
    }
    let mut ip_to_outbound_nids: HashMap<IpAddr, Vec<String>> = HashMap::new();
    for p in &raw.outbound {
        if let Ok(addr) = p.addr.parse::<SocketAddr>() {
            let nid = p
                .node_id
                .clone()
                .or_else(|| by_seed.get(&addr).map(|s| (*s).to_string()));
            if let Some(nid) = nid {
                ip_to_outbound_nids.entry(addr.ip()).or_default().push(nid);
            }
        }
    }
    let resolve_nid =
        |p: &PeerInfo, seen: &HashSet<String>| -> Option<String> {
            if let Some(ref nid) = p.node_id {
                return Some(nid.clone());
            }
            if let Ok(addr) = p.addr.parse::<SocketAddr>() {
                if let Some(nid) = by_seed.get(&addr) {
                    return Some((*nid).to_string());
                }
                if let Some(candidates) = by_ip.get(&addr.ip()) {
                    if candidates.len() == 1 {
                        return Some(candidates[0].to_string());
                    }
                    if let Some(out_nids) = ip_to_outbound_nids.get(&addr.ip()) {
                        let unseen: Vec<&str> = out_nids
                            .iter()
                            .filter(|n| !seen.contains(n.as_str()))
                            .map(|s| s.as_str())
                            .collect();
                        if unseen.len() == 1 {
                            return Some(unseen[0].to_string());
                        }
                    }
                }
            }
            None
        };
    let group =
        |peers: Vec<PeerInfo>| -> Vec<templates::NodeConnectionGroup> {
            let mut seen = HashSet::new();
            let mut result = Vec::new();
            for p in peers {
                let nid = match resolve_nid(&p, &seen) {
                    Some(n) => n,
                    None => continue,
                };
                if nid == local_node_id || !seen.insert(nid.clone()) {
                    continue;
                }
                let mut passed: Vec<String> = Vec::new();
                for ip in &p.wan_ips {
                    passed.push(format!("{} (wan)", ip));
                }
                for ip in &p.intranet_ips {
                    passed.push(format!("{} (intranet)", ip));
                }
                result.push(templates::NodeConnectionGroup {
                    node_id: nid,
                    addrs: p.addr.clone(),
                    passed: passed.join(", "),
                });
            }
            result
        };
    templates::ConnectionsByDirection {
        inbound: group(raw.inbound),
        outbound: group(raw.outbound),
    }
}

// ===================== API handler functions =====================

pub async fn handle_transfer(
    ctx: &mut Context,
    _db: &DatabaseConnection,
    addr: &str,
    transfer_fn: super::types::TransferFn,
) -> bool {
    let (cl, body_bytes) = read_http_body(ctx).await;
    let transfer_req: serde_json::Value =
        serde_json::from_slice(&body_bytes[..cl]).unwrap_or_default();
    let to = transfer_req
        .get("to")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let amount = transfer_req
        .get("amount")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let result = if to.is_empty() || amount == 0 {
        Err(anyhow::anyhow!("Missing 'to' or 'amount'"))
    } else {
        (transfer_fn)(addr.to_string(), to.to_string(), amount.to_string()).await
    };
    let json = match &result {
        Ok(tx_hash) => serde_json::json!({"success": true, "tx_hash": tx_hash}),
        Err(e) => serde_json::json!({"success": false, "error": e.to_string()}),
    };
    ctx.send(json.to_string(), Some(SubMediaType::Json));
    true
}

pub async fn handle_address_api(
    ctx: &mut Context,
    db: &DatabaseConnection,
    meta_path: &str,
) -> bool {
    use crate::db::entity::balance::store::BalanceStore;
    use crate::db::entity::transaction::store::TransactionStore;
    use crate::db::store::DatabaseStore;
    let raw = get_query_param(meta_path, "address").unwrap_or("");
    let addr_str = url_decode_query(raw);
    let balance_store = DatabaseStore::store::<BalanceStore<_>>(db).await;
    let tx_store = DatabaseStore::store::<TransactionStore<_>>(db).await;
    let balance = balance_store.get(&addr_str).await.unwrap_or(0);
    let all_txs = tx_store.get_all().await.unwrap_or_default();
    let txs: Vec<_> = all_txs
        .into_iter()
        .filter(|tx| tx.from_address == addr_str || tx.to_address == addr_str)
        .take(50)
        .collect();
    let tx_infos: Vec<serde_json::Value> = txs
        .into_iter()
        .map(|tx| {
            let tx_type = match tx.tx_type {
                crate::db::entity::transaction::entity::TxType::Transfer => {
                    "transfer"
                }
                crate::db::entity::transaction::entity::TxType::Mint => {
                    "mint"
                }
                crate::db::entity::transaction::entity::TxType::Burn => {
                    "burn"
                }
            };
            serde_json::json!({
                "id": tx.id,
                "tx_type": tx_type,
                "from_address": tx.from_address,
                "to_address": tx.to_address,
                "amount": tx.amount,
                "timestamp": tx.timestamp,
                "hash": tx.hash,
                "pre_hash": tx.pre_hash,
            })
        })
        .collect();
    let json =
        serde_json::json!({"address": addr_str, "balance": balance, "transactions": tx_infos});
    ctx.send(json.to_string(), Some(SubMediaType::Json));
    true
}

pub async fn handle_list_contacts(
    ctx: &mut Context,
    db: &DatabaseConnection,
    addr: &str,
    gctx: Arc<GlobalContext>,
    user_store: &UserStore,
) -> bool {
    use crate::db::entity::contact::store::ContactStore;
    let contact_store = ContactStore::new(db);
    let db_contacts = contact_store.get_all().await.unwrap_or_default();
    let unread_vec = user_store
        .get_all_contacts_with_unread()
        .await
        .unwrap_or_default();
    let unread_by_addr: std::collections::HashMap<String, u64> = unread_vec.into_iter().collect();
    let my_addr = addr;
    let mut contact_map: std::collections::BTreeMap<String, serde_json::Value> =
        std::collections::BTreeMap::new();
    for c in &db_contacts {
        if c.address == my_addr {
            continue;
        }
        let unread = unread_by_addr.get(&c.address).copied().unwrap_or(0);
        let profile = user_store
            .load_profile(&c.address)
            .await
            .unwrap_or_default();
        contact_map.insert(
            c.address.clone(),
            serde_json::json!({
                "id": c.id,
                "name": c.name,
                "address": c.address,
                "created_at": c.created_at,
                "unread_count": unread,
                "nickname": profile.nickname,
                "avatar_path": profile.avatar_path,
            }),
        );
    }
    if let Some(gctx_node) = gctx.get::<Arc<Node>>().await {
        let registry = &gctx_node.registry;
        registry.sync_from_connection_info(&gctx.get_connection_info().await);
        for entry in registry.get_nodes() {
            if entry.address == my_addr {
                continue;
            }
            if !contact_map.contains_key(&entry.address) {
                let last_part = entry.address.rsplit(':').next().unwrap_or(&entry.address);
                let profile = user_store
                    .load_profile(&entry.address)
                    .await
                    .unwrap_or_default();
                contact_map.insert(
                    entry.address.clone(),
                    serde_json::json!({
                        "id": null,
                        "name": last_part,
                        "address": entry.address,
                        "created_at": null,
                        "unread_count": 0,
                        "from_registry": true,
                        "nickname": profile.nickname,
                        "avatar_path": profile.avatar_path,
                    }),
                );
            }
        }
    }
    let contacts_json: Vec<serde_json::Value> = contact_map.into_values().collect();
    let json = serde_json::json!({"success": true, "contacts": contacts_json});
    ctx.send(json.to_string(), Some(SubMediaType::Json));
    true
}

pub async fn handle_add_contact(ctx: &mut Context, db: &DatabaseConnection) -> bool {
    let (cl, body_bytes) = read_http_body(ctx).await;
    let contact_req: serde_json::Value =
        serde_json::from_slice(&body_bytes[..cl]).unwrap_or_default();
    let name = contact_req
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let address = contact_req
        .get("address")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if name.is_empty() || address.is_empty() {
        let json = serde_json::json!({"success": false, "error": "Missing 'name' or 'address'"});
        ctx.send(json.to_string(), Some(SubMediaType::Json));
        return true;
    }
    use crate::db::entity::contact::store::ContactStore;
    let store = ContactStore::new(db);
    let result = store.add(name, address).await;
    let json = match &result {
        Ok(contact) => {
            serde_json::json!({"success": true, "contact": {"id": contact.id, "name": contact.name, "address": contact.address}})
        }
        Err(e) => serde_json::json!({"success": false, "error": e.to_string()}),
    };
    ctx.send(json.to_string(), Some(SubMediaType::Json));
    true
}

pub async fn handle_delete_contact(
    ctx: &mut Context,
    db: &DatabaseConnection,
    meta_path: &str,
) -> bool {
    use crate::db::entity::contact::store::ContactStore;
    let raw = get_query_param(meta_path, "address").unwrap_or("");
    let addr = url_decode_query(raw);
    let store = ContactStore::new(db);
    let result = store.delete(&addr).await;
    let json = match result {
        Ok(_) => serde_json::json!({"success": true}),
        Err(e) => serde_json::json!({"success": false, "error": e.to_string()}),
    };
    ctx.send(json.to_string(), Some(SubMediaType::Json));
    true
}

pub async fn handle_get_chat_messages(
    ctx: &mut Context,
    user_store: &UserStore,
    meta_path: &str,
) -> bool {
    let raw = get_query_param(meta_path, "contact").unwrap_or("");
    let addr = url_decode_query(raw);
    let messages = user_store.get_messages(&addr).await.unwrap_or_default();
    let _ = user_store.mark_as_read(&addr).await;
    let messages_json: Vec<serde_json::Value> = messages
        .into_iter()
        .map(|m| {
            serde_json::json!({
                "id": m.id,
                "contact_address": m.contact_address,
                "content": m.content,
                "is_sent": m.is_sent,
                "status": m.status,
                "timestamp": m.timestamp,
            })
        })
        .collect();
    let json = serde_json::json!({"success": true, "messages": messages_json});
    ctx.send(json.to_string(), Some(SubMediaType::Json));
    true
}

pub async fn handle_get_conversations(ctx: &mut Context, user_store: &UserStore) -> bool {
    let conversations = user_store.get_conversations().await.unwrap_or_default();
    let json = serde_json::json!({"success": true, "conversations": conversations});
    ctx.send(json.to_string(), Some(SubMediaType::Json));
    true
}

async fn embed_avatar(
    store: &UserStore,
    addr: &str,
    mut profile: crate::user_store::UserProfile,
) -> crate::user_store::UserProfile {
    if let Some(ref path) = profile.avatar_path.clone() {
        if let Ok(Some(data)) = store.read_file(addr, path).await {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
            profile.avatar_path = Some(format!("data:image/jpeg;base64,{}", b64));
        }
    }
    profile
}

pub async fn handle_get_profile(
    ctx: &mut Context,
    user_store: &UserStore,
    meta_path: &str,
) -> bool {
    let raw = get_query_param(meta_path, "address").unwrap_or("");
    let addr = url_decode_query(raw);
    let profile = user_store.load_profile(&addr).await.unwrap_or_default();
    let profile = embed_avatar(user_store, &addr, profile).await;
    let json = serde_json::json!({"success": true, "profile": profile});
    ctx.send(json.to_string(), Some(SubMediaType::Json));
    true
}

pub async fn handle_get_self_profile(
    ctx: &mut Context,
    user_store: &UserStore,
    addr: &str,
) -> bool {
    let profile = user_store.load_profile(addr).await.unwrap_or_default();
    let profile = embed_avatar(user_store, addr, profile).await;
    let json = serde_json::json!({"success": true, "profile": profile});
    ctx.send(json.to_string(), Some(SubMediaType::Json));
    true
}

pub async fn handle_save_profile(
    ctx: &mut Context,
    _db: &DatabaseConnection,
    user_store: &UserStore,
    addr: &str,
    meta_path: &str,
) -> bool {
    let (cl, body_bytes) = read_http_body(ctx).await;
    let target = get_query_param(meta_path, "address").unwrap_or(addr);
    if let Ok(mut profile) = serde_json::from_slice::<
        crate::user_store::UserProfile,
    >(&body_bytes[..cl])
    {
        if profile.avatar_path.is_none() {
            if let Ok(existing) = user_store.load_profile(target).await {
                profile.avatar_path = existing.avatar_path;
            }
        }
        let _ = user_store.save_profile(target, &profile).await;
        let json = serde_json::json!({"success": true});
        ctx.send(json.to_string(), Some(SubMediaType::Json));
    } else {
        let json = serde_json::json!({"success": false, "error": "Invalid profile JSON"});
        ctx.send(json.to_string(), Some(SubMediaType::Json));
    }
    true
}

pub async fn handle_upload_avatar(
    ctx: &mut Context,
    user_store: &UserStore,
    addr: &str,
    meta_path: &str,
) -> bool {
    let (_cl, body_bytes) = read_http_body(ctx).await;
    let target = get_query_param(meta_path, "address").unwrap_or(addr);
    let name = "avatar.jpg";
    if let Err(e) = user_store.save_image(target, name, &body_bytes).await {
        let json = serde_json::json!({"success": false, "error": e.to_string()});
        ctx.send(json.to_string(), Some(SubMediaType::Json));
    } else {
        if let Ok(mut profile) = user_store.load_profile(target).await {
            profile.avatar_path = Some(format!("images/{}", name));
            let _ = user_store.save_profile(target, &profile).await;
        }
        let json = serde_json::json!({"success": true, "avatar_path": format!("images/{}", name)});
        ctx.send(json.to_string(), Some(SubMediaType::Json));
    }
    true
}

pub async fn handle_send_chat(
    ctx: &mut Context,
    context: Arc<GlobalContext>,
    addr: &str,
    user_store: Arc<UserStore>,
) -> bool {
    use crate::web::aex_re_exports::WsSenderList;
    use crate::protocols::commands::message::{PendingAcks, next_request_id, send_text_message};
    const ACK_TIMEOUT_SECS: u64 = 30;
    let (cl, body_bytes) = read_http_body(ctx).await;
    let send_req: serde_json::Value = serde_json::from_slice(&body_bytes[..cl]).unwrap_or_default();
    let to = send_req.get("to").and_then(|v| v.as_str()).unwrap_or("");
    let content = send_req
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if to.is_empty() || content.is_empty() {
        let json = serde_json::json!({"success": false, "error": "Missing 'to' or 'message'"});
        ctx.send(json.to_string(), Some(SubMediaType::Json));
        return true;
    }
    let to_addr = to.to_string();
    let msg_body = content.to_string();
    let request_id = next_request_id();
    let target_addrs: Vec<std::net::SocketAddr> = {
        let gctx_node = context.get::<Arc<Node>>().await;
        if let Some(ref node) = gctx_node {
            let addrs = node.registry.get_seeds_for_node(&to_addr);
            if !addrs.is_empty() {
                tracing::info!(
                    "🔍 Found target {} in NodeRegistry, seeds: {:?}",
                    to_addr,
                    addrs
                );
            } else {
                tracing::warn!(
                    "🔍 Target {} NOT found in NodeRegistry, will broadcast to all peers",
                    to_addr
                );
            }
            addrs
        } else {
            tracing::error!("❌ Node not available in context!");
            vec![]
        }
    };
    let (ack_tx, ack_rx) = tokio::sync::oneshot::channel::<bool>();
    {
        if let Some(pending) = context.get::<PendingAcks>().await {
            let mut guard = pending.lock().await;
            guard.insert(request_id, ack_tx);
        } else {
            tracing::warn!("⚠️ No PendingAcks in context, message will be fire-and-forget");
        }
    }
    if let Err(e) = user_store
        .add_message(&to_addr, &msg_body, true, "sent")
        .await
    {
        tracing::error!("Failed to store outgoing message: {}", e);
    }
    {
        if let Some(senders) = context.get::<WsSenderList>().await {
            let preview = if msg_body.len() > 50 {
                format!("{}...", &msg_body[..50])
            } else {
                msg_body.clone()
            };
            let event = serde_json::json!({
                "type": "chat_update",
                "contact": to_addr,
                "preview": preview,
            });
            let _ = senders.broadcast(&event.to_string()).await;
        }
    }
    let manager = context.manager.clone();
    let send_succeeded = Arc::new(std::sync::atomic::AtomicBool::new(false));
    for seed_addr in &target_addrs {
        if let Some(entry) = manager.find_entry(seed_addr) {
            if let Some(send_ctx) = &entry.context {
                match send_text_message(
                    addr.to_string(),
                    to_addr.clone(),
                    request_id,
                    send_ctx.clone(),
                    &msg_body,
                )
                .await
                {
                    Ok(_) => {
                        tracing::info!(
                            "✅ Message sent directly to {} (seed={})",
                            entry.addr,
                            seed_addr
                        );
                        send_succeeded.store(true, std::sync::atomic::Ordering::Relaxed);
                        break;
                    }
                    Err(e) => {
                        tracing::error!("❌ Direct send failed to {}: {:?}", entry.addr, e);
                    }
                }
            }
        }
    }
    if !send_succeeded.load(std::sync::atomic::Ordering::Relaxed) && !target_addrs.is_empty() {
        tracing::warn!(
            "⚠️ Target {} in registry but no matching connection, fallback to forward",
            to_addr
        );
    }
    if !send_succeeded.load(std::sync::atomic::Ordering::Relaxed) {
        let local_addr = addr.to_string();
        let to_addr_clone = to_addr.clone();
        let msg_body_clone = msg_body.clone();
        manager.forward({
            let send_succeeded = send_succeeded.clone();
            let target_addrs = target_addrs.clone();
            move |entries| {
                let msg_body = msg_body_clone.clone();
                let target_addrs = target_addrs.clone();
                let to_addr = to_addr_clone.clone();
                let local_addr = local_addr.clone();
                let send_succeeded = send_succeeded.clone();
                async move {
                    let mut sent_nodes: std::collections::HashSet<String> = std::collections::HashSet::new();
                    for entry in entries {
                        if !target_addrs.is_empty() {
                            let matches_socket = target_addrs.contains(&entry.addr);
                            let matches_peer = entry.context.as_ref().and_then(|ctx| {
                                ctx.try_lock().ok().and_then(|g| g.get::<String>())
                            }).map(|addr| addr == to_addr).unwrap_or(false);
                            if !matches_socket && !matches_peer {
                                continue;
                            }
                        }
                        let node_id = entry.node.read().await;
                        if let Some(n) = node_id.as_ref() {
                            let nid = String::from_utf8_lossy(&n.id).to_string();
                            if !sent_nodes.insert(nid) {
                                tracing::info!("  ⏭️  Skipping duplicate send to {} (already sent to node)", entry.addr);
                                continue;
                            }
                        }
                        if let Some(send_ctx) = &entry.context {
                            match send_text_message(local_addr.to_string(), to_addr.clone(), request_id, send_ctx.clone(), &msg_body).await {
                                Ok(_) => {
                                    tracing::info!("✅ Message sent to {}", entry.addr);
                                    send_succeeded.store(true, std::sync::atomic::Ordering::Relaxed);
                                }
                                Err(e) => {
                                    tracing::error!("❌ Failed to send message to {}: {:?}", entry.addr, e);
                                }
                            }
                        }
                    }
                }
            }
        }).await;
    }
    let sent_ok = send_succeeded.load(std::sync::atomic::Ordering::Relaxed);
    if !sent_ok {
        tracing::error!("❌ No message was sent successfully, aborting");
        {
            if let Some(pending) = context.get::<PendingAcks>().await {
                let mut guard = pending.lock().await;
                guard.remove(&request_id);
            }
        }
        let json = serde_json::json!({"success": false, "error": "Failed to send message: no matching connection or send error (check server logs)"});
        ctx.send(json.to_string(), Some(SubMediaType::Json));
        return true;
    }
    let json = serde_json::json!({"success": true, "message": "Message sent"});
    ctx.send(json.to_string(), Some(SubMediaType::Json));
    let context_bg = context.clone();
    let user_store_bg = user_store.clone();
    let to_addr_bg = to_addr.clone();
    let msg_body_bg = msg_body.clone();
    tokio::spawn(async move {
        let delivered =
            tokio::time::timeout(std::time::Duration::from_secs(ACK_TIMEOUT_SECS), ack_rx).await;
        if let Some(pending) = context_bg.get::<PendingAcks>().await {
            let mut guard = pending.lock().await;
            guard.remove(&request_id);
        }
        match delivered {
            Ok(Ok(true)) => {
                tracing::info!(
                    "✅ Message {} delivered to target {}",
                    request_id,
                    to_addr_bg
                );
                if let Err(e) = user_store_bg
                    .update_last_sent_status(&to_addr_bg, "delivered")
                    .await
                {
                    tracing::error!("Failed to update message status: {}", e);
                }
                if let Some(senders) = context_bg.get::<WsSenderList>().await {
                    let preview = if msg_body_bg.len() > 50 {
                        format!("{}...", &msg_body_bg[..50])
                    } else {
                        msg_body_bg.clone()
                    };
                    let event = serde_json::json!({
                        "type": "chat_message",
                        "contact": to_addr_bg,
                        "preview": preview,
                        "timestamp": Utc::now().timestamp_millis() as u128,
                    });
                    let _ = senders.broadcast(&event.to_string()).await;
                }
            }
            Ok(Ok(false)) => {
                tracing::warn!(
                    "❌ Message {} rejected by recipient {}",
                    request_id,
                    to_addr_bg
                );
            }
            Ok(Err(_)) => {
                tracing::warn!(
                    "❌ Message {} ack channel closed for {}",
                    request_id,
                    to_addr_bg
                );
            }
            Err(_) => {
                tracing::warn!(
                    "⏰ Message {} timed out waiting for ACK from {}",
                    request_id,
                    to_addr_bg
                );
            }
        }
    });
    true
}

// ===================== Page rendering functions =====================

/// Gather all data needed to render pages
async fn gather_page_data(
    db: &DatabaseConnection,
    gctx: Arc<GlobalContext>,
    context: Arc<GlobalContext>,
    minter_data: &super::types::MinterData,
    addr: &str,
    port: u16,
) -> PageData {
    let raw_connections = context.get_connection_info().await;
    let gctx_node = gctx.get::<Arc<Node>>().await;
    let node_registry = gctx_node.as_ref().map(|n| &n.registry);
    if let Some(reg) = &node_registry {
        reg.sync_from_connection_info(&raw_connections);
    }
    let connection_groups = node_registry
        .map(|reg| group_connections_by_direction(raw_connections, reg, addr))
        .unwrap_or_default();
    let seeds = load_all_seeds(db, gctx.clone()).await;
    let nodes = {
        let gctx_node = gctx.get::<Arc<Node>>().await;
        if let Some(node) = gctx_node {
            node.registry
                .get_nodes()
                .into_iter()
                .map(|entry| {
                    let mut intranet_ips = Vec::new();
                    let mut wan_ips = Vec::new();
                    for (addr, _) in &entry.seeds {
                        if addr.ip().is_loopback() || addr.ip().is_unspecified() || addr.port() == 0
                        {
                            continue;
                        }
                        let ip_str = format!("{}:{}", addr.ip(), addr.port());
                        match NetworkScope::from_ip(&addr.ip()) {
                            NetworkScope::Intranet => intranet_ips.push(ip_str),
                            NetworkScope::Extranet => wan_ips.push(ip_str),
                        }
                    }
                    templates::NodeInfo {
                        address: entry.address,
                        intranet_ips,
                        wan_ips,
                        is_connected: entry.is_connected,
                    }
                })
                .collect()
        } else {
            vec![]
        }
    };
    let resources = context
        .get::<ResourceInfo>()
        .await
        .unwrap_or_else(ResourceInfo::detect);
    let (
        cycle_id,
        tick_count,
        epoch_tick,
        ticks_per_epoch,
        witness_ring_active,
        witness_ring_locked,
    ) = {
        let cycle_id = minter_data.cycle_id;
        let tick_count = minter_data.tick_count;
        let epoch_tick = minter_data.epoch_tick;
        let ticks_per_epoch = minter_data.ticks_per_epoch;

        fn ring_to_info(
            hash: &str,
            epoch: i64,
            members: &[super::types::SeedSnapshot],
        ) -> WitnessRingInfo {
            WitnessRingInfo {
                ring_hash: hash.to_string(),
                epoch,
                member_count: members.len(),
                members: members
                    .iter()
                    .map(|s| templates::WitnessRingMemberInfo {
                        ip: s.ip.clone(),
                        port: s.port,
                        node_id: s.node_id.clone(),
                        is_active: s.is_active,
                    })
                    .collect(),
            }
        }

        let active = if !minter_data.active_ring_hash.is_empty() {
            ring_to_info(
                &minter_data.active_ring_hash,
                minter_data.active_ring_epoch,
                &minter_data.active_ring_members,
            )
        } else {
            WitnessRingInfo::default()
        };
        let locked = if !minter_data.locked_ring_hash.is_empty() {
            ring_to_info(
                &minter_data.locked_ring_hash,
                minter_data.locked_ring_epoch,
                &minter_data.locked_ring_members,
            )
        } else {
            WitnessRingInfo::default()
        };
        (
            cycle_id,
            tick_count,
            epoch_tick,
            ticks_per_epoch,
            active,
            locked,
        )
    };
    let (witness_tick_records, witness_tick_max) = {
        use crate::db::entity::witness_tick_record::store::WitnessTickRecordStore;
        let store = WitnessTickRecordStore::new(db);
        let max_tick = store.get_max_tick_count(cycle_id).await.unwrap_or(0);
        let records = store.get_epoch_records(cycle_id).await.unwrap_or_default();
        let infos: Vec<templates::WitnessTickRecordInfo> = records
            .into_iter()
            .map(|r| templates::WitnessTickRecordInfo {
                address: r.address,
                tick_count: r.tick_count,
                is_full_time: max_tick > 0 && r.tick_count >= max_tick,
            })
            .collect();
        (infos, max_tick)
    };
    let status = {
        let mut s = templates::StatusInfo::new(tick_count as u8, cycle_id);
        s.epoch_tick = epoch_tick;
        s.ticks_per_epoch = ticks_per_epoch;
        s
    };
    let witness_table = {
        let mut info = WitnessTableInfo::new(tick_count as u8);
        info.active_count = minter_data.cycle_id as u64;
        info.pending_count = 0;
        for w in &minter_data.witness_entries {
            info.witnesses.push(templates::WitnessInfo {
                address: w.address.clone(),
                is_online: w.is_online,
                tick_count: w.tick_count,
                weight: w.weight.clone(),
                validated_by: w.validated_by.clone(),
            });
        }
        info
    };
    let account = context
        .get::<AccountInfo>()
        .await
        .unwrap_or_else(|| AccountInfo::new(addr.to_string()));
    let transactions = load_transactions(db, 1, 20).await;
    let (inner_ips, external_ips) = crate::protocols::commands::online::get_all_ips();
    let my_balance = {
        use crate::db::entity::balance::store::BalanceStore;
        use crate::db::store::DatabaseStore;
        let store = DatabaseStore::store::<BalanceStore<DatabaseConnection>>(db).await;
        store.get(addr).await.unwrap_or(0)
    };
    let witness_chain = minter_data.witness_chain.clone();

    PageData {
        connection_groups,
        seeds,
        nodes,
        resources,
        epoch_tick,
        ticks_per_epoch,
        witness_ring_active,
        witness_ring_locked,
        witness_tick_records,
        witness_tick_max,
        status,
        witness_table,
        account,
        transactions,
        inner_ips: inner_ips
            .into_iter()
            .map(|ip| format!("{}:{}", ip, port))
            .collect(),
        external_ips: external_ips
            .into_iter()
            .map(|ip| format!("{}:{}", ip, port))
            .collect(),
        my_balance,
        witness_chain,
    }
}

struct PageData {
    connection_groups: templates::ConnectionsByDirection,
    seeds: Vec<templates::SeedInfo>,
    nodes: Vec<templates::NodeInfo>,
    resources: ResourceInfo,
    epoch_tick: u64,
    ticks_per_epoch: u64,
    witness_ring_active: WitnessRingInfo,
    witness_ring_locked: WitnessRingInfo,
    witness_tick_records: Vec<templates::WitnessTickRecordInfo>,
    witness_tick_max: u8,
    status: templates::StatusInfo,
    witness_table: WitnessTableInfo,
    account: AccountInfo,
    transactions: TransactionPage,
    inner_ips: Vec<String>,
    external_ips: Vec<String>,
    my_balance: u64,
    witness_chain: std::collections::HashMap<String, serde_json::Value>,
}

pub async fn handle_data_api(
    ctx: &mut Context,
    db: &DatabaseConnection,
    gctx: Arc<GlobalContext>,
    context: Arc<GlobalContext>,
    minter_data: &super::types::MinterData,
    addr: &str,
    port: u16,
) -> bool {
    let d = gather_page_data(db, gctx, context, minter_data, addr, port).await;
    let json = serde_json::json!({
        "tick_count": d.status.tick_count,
        "today_tick": d.status.today_tick,
        "next_tick_seconds": d.status.next_tick_seconds,
        "epoch": d.status.epoch,
        "epoch_tick": d.epoch_tick,
        "ticks_per_epoch": d.ticks_per_epoch,
        "witness_ring_active": d.witness_ring_active,
        "witness_ring_locked": d.witness_ring_locked,
        "witness_tick_records": d.witness_tick_records,
        "witness_tick_max": d.witness_tick_max,
        "witness_chain": d.witness_chain,
        "inbound_connections": d.connection_groups.inbound,
        "outbound_connections": d.connection_groups.outbound,
        "nodes": d.nodes,
        "seeds": d.seeds,
        "my_address": addr,
        "my_balance": d.my_balance,
    });
    ctx.send(json.to_string(), Some(SubMediaType::Json));
    true
}

pub async fn handle_wallet_page(
    ctx: &mut Context,
    port: u16,
    name: &str,
    addr: &str,
    context: Arc<GlobalContext>,
    minter_data: &super::types::MinterData,
    db: &DatabaseConnection,
    gctx: Arc<GlobalContext>,
) -> bool {
    let d = gather_page_data(db, gctx, context, minter_data, addr, port).await;
    let res = WalletTemplate {
        p2p_port: port,
        node_name: name.to_string(),
        node_address: addr.to_string(),
        explorer_url: DEFAULT_EXPLORER_URL.to_string(),
        account: d.account,
        status: d.status,
    }
    .render()
    .unwrap_or_default();
    ctx.send(&res, None);
    true
}

pub async fn handle_chat_page(
    ctx: &mut Context,
    port: u16,
    name: &str,
    addr: &str,
    context: Arc<GlobalContext>,
    minter_data: &super::types::MinterData,
    db: &DatabaseConnection,
    gctx: Arc<GlobalContext>,
) -> bool {
    let d = gather_page_data(db, gctx, context, minter_data, addr, port).await;
    let res = ChatTemplate {
        web_port: port,
        p2p_port: port,
        node_name: name.to_string(),
        node_address: addr.to_string(),
        explorer_url: DEFAULT_EXPLORER_URL.to_string(),
        account: d.account,
        status: d.status,
    }
    .render()
    .unwrap_or_default();
    ctx.send(&res, None);
    true
}

pub async fn handle_network_page(
    ctx: &mut Context,
    port: u16,
    name: &str,
    dir: &str,
    db: &DatabaseConnection,
    gctx: Arc<GlobalContext>,
    context: Arc<GlobalContext>,
    minter_data: &super::types::MinterData,
    addr: &str,
) -> bool {
    let d = gather_page_data(db, gctx, context, minter_data, addr, port).await;
    let protocols = "tcp, udp, http, ws";
    let res = NetworkTemplate {
        web_port: port,
        p2p_port: port,
        node_name: name.to_string(),
        data_dir: dir.to_string(),
        inner_ips: d.inner_ips,
        external_ips: d.external_ips,
        node_address: addr.to_string(),
        protocols: protocols.to_string(),
        explorer_url: DEFAULT_EXPLORER_URL.to_string(),
        inbound_connections: d.connection_groups.inbound,
        outbound_connections: d.connection_groups.outbound,
        status: d.status,
        resources: d.resources,
        witness_table: d.witness_table,
        witness_ring_active: d.witness_ring_active,
        witness_ring_locked: d.witness_ring_locked,
        witness_tick_records: d.witness_tick_records,
        witness_tick_max: d.witness_tick_max as u8,
        transactions: d.transactions,
        seeds: d.seeds,
        nodes: d.nodes,
    }
    .render()
    .unwrap_or_default();
    ctx.send(&res, None);
    true
}

pub async fn handle_index_page(
    ctx: &mut Context,
    port: u16,
    name: &str,
    dir: &str,
    addr: &str,
    context: Arc<GlobalContext>,
    minter_data: &super::types::MinterData,
    db: &DatabaseConnection,
    gctx: Arc<GlobalContext>,
) -> bool {
    let d = gather_page_data(db, gctx, context, minter_data, addr, port).await;
    let protocols = "tcp, udp, http, ws";
    let res = templates::IndexTemplate {
        web_port: port,
        p2p_port: port,
        node_name: name.to_string(),
        data_dir: dir.to_string(),
        inner_ips: d.inner_ips,
        external_ips: d.external_ips,
        node_address: addr.to_string(),
        protocols: protocols.to_string(),
        explorer_url: DEFAULT_EXPLORER_URL.to_string(),
        status: d.status,
        resources: d.resources,
    }
    .render()
    .unwrap_or_default();
    ctx.send(&res, None);
    true
}
