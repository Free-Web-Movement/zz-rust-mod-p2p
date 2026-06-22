use askama::Template;
use chrono::Timelike;
use serde::{Deserialize, Serialize};

pub const DEFAULT_EXPLORER_URL: &str = "https://explorer.freewebmovement.com";
pub const TICK_INTERVAL_MINUTES: u32 = 15;
pub const TICKS_PER_EPOCH: u64 = 96;

#[derive(Template)]
#[template(path = "index.html")]
#[allow(dead_code)]
pub struct IndexTemplate {
    pub web_port: u16,
    pub p2p_port: u16,
    pub node_name: String,
    pub data_dir: String,
    pub inner_ips: Vec<String>,
    pub external_ips: Vec<String>,
    pub node_address: String,
    pub protocols: String,
    pub explorer_url: String,
    pub status: StatusInfo,
    pub resources: ResourceInfo,
}

#[derive(Template)]
#[template(path = "network.html")]
#[allow(dead_code)]
pub struct NetworkTemplate {
    pub web_port: u16,
    pub p2p_port: u16,
    pub node_name: String,
    pub data_dir: String,
    pub inner_ips: Vec<String>,
    pub external_ips: Vec<String>,
    pub node_address: String,
    pub protocols: String,
    pub explorer_url: String,
    pub status: StatusInfo,
    pub resources: ResourceInfo,
    pub witness_table: WitnessTableInfo,
    pub witness_ring_active: WitnessRingInfo,
    pub witness_ring_locked: WitnessRingInfo,
    pub witness_tick_records: Vec<WitnessTickRecordInfo>,
    pub witness_tick_max: u8,
    pub transactions: TransactionPage,
    pub seeds: Vec<SeedInfo>,
    pub nodes: Vec<NodeInfo>,
    pub inbound_connections: Vec<NodeConnectionGroup>,
    pub outbound_connections: Vec<NodeConnectionGroup>,
}

#[derive(Template)]
#[template(path = "chat.html")]
#[allow(dead_code)]
pub struct ChatTemplate {
    pub web_port: u16,
    pub p2p_port: u16,
    pub node_name: String,
    pub node_address: String,
    pub explorer_url: String,
    pub account: AccountInfo,
    pub status: StatusInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedInfo {
    pub address: String,
    pub port: u16,
    pub protocol: String,
    pub is_active: bool,
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub address: String,
    pub intranet_ips: Vec<String>,
    pub wan_ips: Vec<String>,
    pub is_connected: bool,
}

/// 按 node_id 分组的连接（同一方向内，一个 node 的所有地址）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConnectionGroup {
    pub node_id: String,
    pub addrs: String,  // connected ip:port（实际 TCP 连接地址）
    pub passed: String, // passed ip:port（对端宣告的地址）
}

/// 区分 inbound / outbound 的连接分组
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectionsByDirection {
    pub inbound: Vec<NodeConnectionGroup>,
    pub outbound: Vec<NodeConnectionGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInfo {
    pub storage_tb: f64,
    pub storage_weight: String,
    pub bandwidth_gbps: f64,
    pub bandwidth_weight: String,
    pub cpu_cores: u64,
    pub cpu_ghz: f64,
    pub cpu_weight: String,
    pub memory_gb: f64,
    pub memory_weight: String,
    pub api_requests: u64,
    pub api_weight: String,
    pub composite_weight: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusInfo {
    pub tick_count: u8,
    pub today_tick: u8,
    pub epoch: i64,
    pub epoch_tick: u64,
    pub ticks_per_epoch: u64,
    pub tick_interval_minutes: u32,
    pub next_tick_seconds: u8,
}
impl StatusInfo {
    pub fn new(tick_count: u8, epoch: i64) -> Self {
        let now = chrono::Utc::now() + chrono::Duration::hours(8);
        let seconds = now.hour() * 3600 + now.minute() * 60 + now.second();
        let tick_interval = 15 * 60;
        let today_tick = seconds / tick_interval;
        let next_tick = tick_interval - (seconds % tick_interval);

        Self {
            tick_count,
            today_tick: today_tick as u8,
            epoch,
            epoch_tick: 0,
            ticks_per_epoch: TICKS_PER_EPOCH,
            tick_interval_minutes: TICK_INTERVAL_MINUTES,
            next_tick_seconds: next_tick as u8,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WitnessRingMemberInfo {
    pub ip: String,
    pub port: i32,
    pub node_id: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WitnessRingInfo {
    pub ring_hash: String,
    pub epoch: i64,
    pub member_count: usize,
    pub members: Vec<WitnessRingMemberInfo>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WitnessInfo {
    pub address: String,
    pub is_online: bool,
    pub tick_count: u8,
    pub weight: String,
    pub validated_by: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WitnessTickRecordInfo {
    pub address: String,
    pub tick_count: u8,
    pub is_full_time: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WitnessTableInfo {
    pub active_count: u64,
    pub pending_count: u64,
    pub witnesses: Vec<WitnessInfo>,
    pub tick_count: u8,
    pub tick_interval_minutes: u32,
    pub next_tick_seconds: u8,
}
impl WitnessTableInfo {
    pub fn new(tick_count: u8) -> Self {
        let now = chrono::Utc::now() + chrono::Duration::hours(8);
        let seconds = now.hour() * 3600 + now.minute() * 60 + now.second();
        let tick_interval = 15 * 60;
        let next_tick = tick_interval - (seconds % tick_interval);

        Self {
            active_count: 0,
            pending_count: 0,
            witnesses: vec![],
            tick_count,
            tick_interval_minutes: 15,
            next_tick_seconds: next_tick as u8,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccountInfo {
    pub address: String,
    pub balance: String,
    pub pending_balance: String,
    pub total_sent: String,
    pub total_received: String,
    pub tx_count: u64,
}

impl AccountInfo {
    pub fn new(address: String) -> Self {
        Self {
            address,
            balance: "0.00".to_string(),
            pending_balance: "0.00".to_string(),
            total_sent: "0.00".to_string(),
            total_received: "0.00".to_string(),
            tx_count: 0,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransactionInfo {
    pub id: i64,
    pub tx_type: String,
    pub from_address: String,
    pub to_address: String,
    pub amount: String,
    pub timestamp: String,
    pub hash: String,
    pub pre_hash: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransactionPage {
    pub transactions: Vec<TransactionInfo>,
    pub current_page: u64,
    pub total_pages: u64,
    pub total_count: u64,
}

impl Default for ResourceInfo {
    fn default() -> Self {
        Self {
            storage_tb: 1.0,
            storage_weight: "1.00".to_string(),
            bandwidth_gbps: 1.0,
            bandwidth_weight: "1.00".to_string(),
            cpu_cores: 4,
            cpu_ghz: 2.0,
            cpu_weight: "0.80".to_string(),
            memory_gb: 8.0,
            memory_weight: "0.80".to_string(),
            api_requests: 1000,
            api_weight: "1.00".to_string(),
            composite_weight: "4.60".to_string(),
        }
    }
}

impl ResourceInfo {
    pub fn detect() -> Self {
        let cpu_cores = detect_cpu_cores() as u64;
        let memory_gb = detect_memory_gb();
        let storage_tb = detect_storage_tb();
        let cpu_ghz = detect_cpu_ghz();

        let storage_weight = format!("{:.2}", storage_tb);
        let bandwidth_weight = format!("{:.2}", 1.0_f64);
        let cpu_w = cpu_cores as f64 * cpu_ghz / 10.0;
        let cpu_weight = format!("{:.2}", cpu_w);
        let memory_weight = format!("{:.2}", memory_gb / 10.0);
        let api_weight = format!("{:.2}", 1000.0_f64 / 1000.0);
        let composite = format!("{:.2}", storage_tb + 1.0 + cpu_w + memory_gb / 10.0 + 1.0);

        Self {
            storage_tb,
            storage_weight,
            bandwidth_gbps: 1.0,
            bandwidth_weight,
            cpu_cores,
            cpu_ghz,
            cpu_weight,
            memory_gb,
            memory_weight,
            api_requests: 1000,
            api_weight,
            composite_weight: composite,
        }
    }
}

fn detect_storage_tb() -> f64 {
    #[cfg(unix)]
    {
        use std::process::Command;
        if let Ok(output) = Command::new("df").arg("-TB").arg("/").output() {
            let output = String::from_utf8_lossy(&output.stdout);
            for line in output.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 6 {
                    if let Ok(size) = parts[3].trim().trim_end_matches('T').parse::<f64>() {
                        return size.max(1.0);
                    }
                }
            }
        }
    }
    1.0
}

fn detect_memory_gb() -> f64 {
    #[cfg(unix)]
    {
        use std::fs;
        if let Ok(content) = fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(kb) = line.split_whitespace().nth(1) {
                        if let Ok(kb_val) = kb.parse::<f64>() {
                            return kb_val / 1024.0 / 1024.0;
                        }
                    }
                }
            }
        }
    }
    8.0
}

fn detect_cpu_cores() -> u64 {
    #[cfg(unix)]
    {
        use std::fs;
        if let Ok(content) = fs::read_to_string("/proc/cpuinfo") {
            let mut count = 0u64;
            for line in content.lines() {
                if line.starts_with("processor") {
                    count += 1;
                }
            }
            if count > 0 {
                return count;
            }
        }
    }
    4
}

fn detect_cpu_ghz() -> f64 {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        if let Ok(content) = fs::read_to_string("/proc/cpuinfo") {
            for line in content.lines() {
                if line.starts_with("cpu MHz") {
                    if let Some(mhz) = line.split(':').nth(1) {
                        if let Ok(mhz_val) = mhz.trim().parse::<f64>() {
                            return mhz_val / 1000.0;
                        }
                    }
                }
            }
        }
    }
    2.0
}

impl Default for IndexTemplate {
    fn default() -> Self {
        Self {
            web_port: 8080,
            p2p_port: 9000,
            node_name: String::from("Unknown"),
            data_dir: String::from("./data"),
            inner_ips: vec![],
            external_ips: vec![],
            node_address: String::from("N/A"),
            protocols: String::from("tcp, udp, http, ws"),
            explorer_url: DEFAULT_EXPLORER_URL.to_string(),
            status: StatusInfo::new(0, 0),
            resources: ResourceInfo::default(),
        }
    }
}

#[derive(Template)]
#[template(path = "wallet.html")]
pub struct WalletTemplate {
    pub p2p_port: u16,
    pub node_name: String,
    pub node_address: String,
    pub explorer_url: String,
    pub account: AccountInfo,
    pub status: StatusInfo,
}
