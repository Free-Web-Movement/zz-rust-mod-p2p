use aex::connection::scope::NetworkScope;
use dashmap::DashMap;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct NodeEntry {
    pub address: String,
    pub seeds: HashSet<SocketAddr>,
    pub is_connected: bool,
    pub scope: NetworkScope,
    pub last_seen: u64,
}

#[derive(Clone)]
pub struct NodeRegistry {
    nodes: Arc<DashMap<String, NodeEntry>>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(DashMap::new()),
        }
    }

    pub fn register(&self, address: String, seed: SocketAddr, scope: NetworkScope) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut entry = self.nodes.entry(address.clone()).or_insert_with(|| NodeEntry {
            address,
            seeds: HashSet::new(),
            is_connected: false,
            scope,
            last_seen: now,
        });

        entry.seeds.insert(seed);
        entry.last_seen = now;
    }

    pub fn add_seed(&self, address: &str, seed: SocketAddr) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if let Some(mut entry) = self.nodes.get_mut(address) {
            let added = entry.seeds.insert(seed);
            entry.last_seen = now;
            added
        } else {
            false
        }
    }

    /// 原子检查并设置连接状态。
    /// 返回 true 表示成功获取连接权（之前未连接），false 表示该节点已连接。
    pub fn try_connect(&self, address: &str) -> bool {
        use dashmap::mapref::entry::Entry;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        match self.nodes.entry(address.to_string()) {
            Entry::Occupied(mut entry) => {
                if entry.get().is_connected {
                    false
                } else {
                    entry.get_mut().is_connected = true;
                    entry.get_mut().last_seen = now;
                    true
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(NodeEntry {
                    address: address.to_string(),
                    seeds: HashSet::new(),
                    is_connected: true,
                    scope: NetworkScope::Intranet,
                    last_seen: now,
                });
                true
            }
        }
    }

    pub fn mark_connected(&self, address: &str, connected: bool) {
        if let Some(mut entry) = self.nodes.get_mut(address) {
            entry.is_connected = connected;
        }
    }

    pub fn is_connected(&self, address: &str) -> bool {
        self.nodes
            .get(address)
            .map(|e| e.is_connected)
            .unwrap_or(false)
    }

    /// 断开节点：清除 is_connected 标志
    pub fn disconnect(&self, address: &str) {
        if let Some(mut entry) = self.nodes.get_mut(address) {
            entry.is_connected = false;
        }
    }

    pub fn is_registered(&self, address: &str) -> bool {
        self.nodes.contains_key(address)
    }

    pub fn get_all_seeds(&self) -> Vec<(SocketAddr, String)> {
        let mut seeds = Vec::new();
        for entry in self.nodes.iter() {
            for seed in entry.seeds.iter() {
                seeds.push((*seed, entry.key().clone()));
            }
        }
        seeds
    }

    pub fn get_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn get_nodes(&self) -> Vec<NodeEntry> {
        self.nodes.iter().map(|e| e.value().clone()).collect()
    }

    pub fn get_connected_nodes(&self) -> Vec<String> {
        self.nodes
            .iter()
            .filter(|e| e.value().is_connected)
            .map(|e| e.key().clone())
            .collect()
    }

    pub fn find_node_for_seed(&self, seed: &SocketAddr) -> Option<String> {
        for entry in self.nodes.iter() {
            if entry.seeds.contains(seed) {
                return Some(entry.key().clone());
            }
        }
        None
    }

    pub fn get_seeds_for_node(&self, address: &str) -> Vec<SocketAddr> {
        self.nodes
            .get(address)
            .map(|e| e.seeds.iter().cloned().collect())
            .unwrap_or_default()
    }
}
