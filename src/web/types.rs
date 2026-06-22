//! Snapshot types for web handlers to access Minter data without a direct dependency.
use std::collections::HashMap;
use std::sync::Arc;

/// Snapshot of Minter fields that the web UI needs.
#[derive(Debug, Clone)]
pub struct MinterData {
    pub cycle_id: i64,
    pub tick_count: u64,
    pub epoch_tick: u64,
    pub ticks_per_epoch: u64,
    pub active_ring_hash: String,
    pub active_ring_epoch: i64,
    pub active_ring_members: Vec<SeedSnapshot>,
    pub locked_ring_hash: String,
    pub locked_ring_epoch: i64,
    pub locked_ring_members: Vec<SeedSnapshot>,
    pub witness_entries: Vec<WitnessEntry>,
    /// Pre-serialized witness chain for the data API JSON response.
    pub witness_chain: HashMap<String, serde_json::Value>,
}

/// Lightweight copy of a ring seed, suitable for templates.
#[derive(Debug, Clone)]
pub struct SeedSnapshot {
    pub ip: String,
    pub port: i32,
    pub node_id: String,
    pub is_active: bool,
}

/// Snapshot of a witness for web display.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WitnessEntry {
    pub address: String,
    pub tick_count: u8,
    pub is_current: bool,
    pub online_minutes: u64,
    pub is_online: bool,
    pub weight: String,
    pub validated_by: Vec<String>,
}

/// Callback for creating a transfer via Minter. Returns the transaction hash.
pub type TransferFn = Arc<
    dyn Fn(
        String,
        String,
        String,
    ) -> std::pin::Pin<Box<dyn futures::Future<Output = anyhow::Result<String>> + Send>>
        + Send
        + Sync,
>;
