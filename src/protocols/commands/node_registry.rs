use aex::connection::scope::NetworkScope;
use dashmap::DashMap;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionDirection {
    Inbound,
    Outbound,
    Unknown,
}

#[derive(Clone)]
pub struct NodeEntry {
    pub address: String,
    pub seeds: HashMap<SocketAddr, ConnectionDirection>,
    pub is_connected: bool,
    pub scope: NetworkScope,
    pub last_seen: u64,
}

pub type NodeManager = NodeRegistry;

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
            seeds: HashMap::new(),
            is_connected: false,
            scope,
            last_seen: now,
        });

        entry.seeds.entry(seed).or_insert(ConnectionDirection::Unknown);
        entry.last_seen = now;
    }

    /// Register a seed with direction info
    pub fn register_with_direction(
        &self,
        address: String,
        seed: SocketAddr,
        scope: NetworkScope,
        direction: ConnectionDirection,
    ) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut entry = self.nodes.entry(address.clone()).or_insert_with(|| NodeEntry {
            address,
            seeds: HashMap::new(),
            is_connected: false,
            scope,
            last_seen: now,
        });

        entry.seeds.insert(seed, direction);
        entry.last_seen = now;
    }

    pub fn add_seed(&self, address: &str, seed: SocketAddr) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if let Some(mut entry) = self.nodes.get_mut(address) {
            let existed = entry.seeds.contains_key(&seed);
            entry.seeds.entry(seed).or_insert(ConnectionDirection::Unknown);
            entry.last_seen = now;
            !existed
        } else {
            false
        }
    }

    /// Atomically check and set connection status.
    /// Returns true if connection was acquired (was not connected), false if already connected.
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
                    seeds: HashMap::new(),
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

    /// Disconnect node: clear is_connected flag
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
            for seed in entry.seeds.keys() {
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
            if entry.seeds.contains_key(seed) {
                return Some(entry.key().clone());
            }
        }
        None
    }

    pub fn get_seeds_for_node(&self, address: &str) -> Vec<SocketAddr> {
        self.nodes
            .get(address)
            .map(|e| e.seeds.keys().cloned().collect())
            .unwrap_or_default()
    }

    pub fn get_inbound_seeds(&self, address: &str) -> Vec<SocketAddr> {
        self.nodes
            .get(address)
            .map(|e| {
                e.seeds
                    .iter()
                    .filter(|(_, dir)| **dir == ConnectionDirection::Inbound)
                    .map(|(addr, _)| *addr)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn get_outbound_seeds(&self, address: &str) -> Vec<SocketAddr> {
        self.nodes
            .get(address)
            .map(|e| {
                e.seeds
                    .iter()
                    .filter(|(_, dir)| **dir == ConnectionDirection::Outbound)
                    .map(|(addr, _)| *addr)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Sync seed direction from AEX ConnectionInfo.
    /// Marks the actual TCP endpoint as Inbound/Outbound per peer.
    pub fn sync_from_connection_info(&self, info: &aex::connection::global::ConnectionInfo) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        for peer in &info.inbound {
            if let Some(ref nid) = peer.node_id {
                if let Ok(addr) = peer.addr.parse::<SocketAddr>() {
                    let scope = NetworkScope::from_ip(&addr.ip());
                    self.nodes
                        .entry(nid.clone())
                        .and_modify(|e| {
                            e.seeds.insert(addr, ConnectionDirection::Inbound);
                            e.scope = scope;
                            e.is_connected = true;
                            e.last_seen = now;
                        })
                        .or_insert(NodeEntry {
                            address: nid.clone(),
                            seeds: HashMap::from([(addr, ConnectionDirection::Inbound)]),
                            is_connected: true,
                            scope,
                            last_seen: now,
                        });
                }
            }
        }

        for peer in &info.outbound {
            if let Some(ref nid) = peer.node_id {
                if let Ok(addr) = peer.addr.parse::<SocketAddr>() {
                    let scope = NetworkScope::from_ip(&addr.ip());
                    self.nodes
                        .entry(nid.clone())
                        .and_modify(|e| {
                            e.seeds.insert(addr, ConnectionDirection::Outbound);
                            e.scope = scope;
                            e.is_connected = true;
                            e.last_seen = now;
                        })
                        .or_insert(NodeEntry {
                            address: nid.clone(),
                            seeds: HashMap::from([(addr, ConnectionDirection::Outbound)]),
                            is_connected: true,
                            scope,
                            last_seen: now,
                        });
                }
            }
        }
    }
}
