use aex::connection::protocol::Protocol;
use aex::storage::Storage;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, hash::Hasher, net::SocketAddr};

use std::hash::Hash;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeRecord {
    /// 使用 SocketAddr 作为唯一网络识别
    pub endpoint: SocketAddr,

    /// 该 endpoint 支持 / 曾成功使用过的协议集合
    pub protocols: HashSet<Protocol>,

    /// 首次发现时间
    pub first_seen: DateTime<Utc>,

    /// 最近一次成功通信
    pub last_seen: DateTime<Utc>,

    /// 连通性评分（记录当前节点连接成功率）(started, ended)
    pub periods: Vec<(DateTime<Utc>, DateTime<Utc>)>,
    /// 连通性评分（记录当前节点连接成功率）(success, failure)
    pub tries: (u64, u64),
    pub is_available: bool,
}

// 手动实现 PartialEq：只要 endpoint 相同，就认为是同一个节点
impl PartialEq for NodeRecord {
    fn eq(&self, other: &Self) -> bool {
        self.endpoint == other.endpoint
    }
}

impl Eq for NodeRecord {}

// 手动实现 Hash：只对 endpoint 进行 hash
impl Hash for NodeRecord {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.endpoint.hash(state);
    }
}

const MAX_VALID_DAYS: i64 = 5;

impl NodeRecord {
    pub fn new(endpoint: SocketAddr) -> Self {
        let now = Utc::now();
        let mut protocols = HashSet::new();
        protocols.insert(Protocol::Http);
        protocols.insert(Protocol::Tcp);

        Self {
            endpoint,
            protocols,
            first_seen: now,
            last_seen: now,
            tries: (0, 0), // 初始成功
            periods: vec![],
            is_available: true,
        }
    }

    /// 更新节点状态
    pub fn update_status(&mut self, success: bool) {
        let now = Utc::now();
        self.last_seen = if success { Utc::now() } else { self.last_seen };
        if success {
            self.last_seen = now;
            self.tries.0 += 1;
            self.is_available = true; // 只要连通一次，自动恢复可用状态
            self.update_periods(now);
        } else {
            self.tries.1 += 1;
        }
    }

    /// 判断是否失效（超过 5 天未见）
    pub fn is_expired(&self) -> bool {
        let now = Utc::now();
        now.signed_duration_since(self.last_seen).num_days() >= MAX_VALID_DAYS
    }

    /// 维护活跃区间
    fn update_periods(&mut self, now: DateTime<Utc>) {
        if let Some(last) = self.periods.last_mut() {
            // 如果距离上次区间结束小于 1 小时，则视为同一段活跃期，仅延长结束时间
            if now.signed_duration_since(last.1).num_minutes() < 60 {
                last.1 = now;
                return;
            }
        }
        // 否则，开启新的活跃区间
        self.periods.push((now, now));
    }

    /// 检查并执行失效标记
    pub fn check_expiry(&mut self) {
        if Utc::now().signed_duration_since(self.last_seen).num_days() >= MAX_VALID_DAYS {
            self.is_available = false;
        }
    }
}

#[derive(Debug, Clone)]
pub struct NodeRegistry {
    pub nodes: HashSet<NodeRecord>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            nodes: HashSet::new(),
        }
    }

    /// 添加或更新节点
    pub fn upsert(&mut self, endpoint: SocketAddr, success: bool) {
        // 尝试从集合中取出已存在的记录
        let mut record = self
            .nodes
            .take(&NodeRecord::new(endpoint))
            .unwrap_or_else(|| NodeRecord::new(endpoint));

        // 更新状态
        record.update_status(success);

        // 放回集合
        self.nodes.insert(record);
    }

    /// 获取可用节点（排除手动标记为失效或逻辑上过期的）
    pub fn get_available_nodes(&self) -> Vec<&NodeRecord> {
        self.nodes
            .iter()
            .filter(|n| n.is_available && !n.is_expired())
            .collect()
    }

    /// 核心逻辑：从 Storage 中恢复数据，并执行启动时的失效检查
    pub fn load_from_storage(storage: &Storage, path: &str) -> Self {
        let nodes = match storage.read::<HashSet<NodeRecord>>(&path.to_string()) {
            Ok(Some(set)) => set,
            _ => HashSet::new(),
        };

        let mut registry = Self { nodes };
        // 关键需求：启动时计算并标记 5 天以上的失效节点
        registry.on_startup_maintenance();
        registry
    }

    /// 执行启动维护：标记逻辑
    pub fn on_startup_maintenance(&mut self) {
        // 由于 HashSet 元素不可变，需要 take 出来修改后再放回
        let old_nodes: Vec<NodeRecord> = self.nodes.drain().collect();
        for mut node in old_nodes {
            // 这里会根据 MAX_VALID_DAYS (5天) 更新 node 的状态或属性
            // 如果你之前定义了 is_available 字段，可以在这里 check_expiry()
            node.check_expiry();

            if node.is_expired() {
                // 可以在这里做特定处理，比如记录日志
            }
            self.nodes.insert(node);
        }
    }

    /// 持久化到 Storage
    pub fn save_to_storage(&self, storage: &Storage, path: &str) -> anyhow::Result<()> {
        storage.save(&path.to_string(), &self.nodes)
    }
}
