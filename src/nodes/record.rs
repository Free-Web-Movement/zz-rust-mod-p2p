use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::SocketAddr;
use std::{collections::HashSet, path::Path};

use crate::consts::DEFAULT_APP_DIR_SERVER_LIST_JSON_FILE;
use crate::protocols::defines::ProtocolCapability;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeRecord {
    /// 使用 SocketAddr 作为唯一网络识别
    pub endpoint: SocketAddr,

    /// 该 endpoint 支持 / 曾成功使用过的协议集合
    pub protocols: ProtocolCapability,

    /// 首次发现时间
    pub first_seen: DateTime<Utc>,

    /// 最近一次成功通信
    pub last_seen: DateTime<Utc>,

    /// 最近一次确认不可达
    pub last_disappeared: Option<DateTime<Utc>>,

    /// 连通性评分（用于节点筛选 / 退避）
    pub reachability_score: f32,
}

impl NodeRecord {
    /// 默认从 {data_dir}/server-list.json 读取
    pub fn load_from_data_dir<P: AsRef<Path>>(data_dir: P) -> Vec<NodeRecord> {
        let path = data_dir
            .as_ref()
            .join(DEFAULT_APP_DIR_SERVER_LIST_JSON_FILE);
        Self::load_from_path(path)
    }

    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Vec<NodeRecord> {
        let path = path.as_ref();

        if !path.exists() {
            // 文件不存在 → 返回空列表（这是正确行为）
            return Vec::new();
        }

        let content = fs::read_to_string(path).unwrap();
        serde_json::from_str(&content).unwrap()
    }

    pub fn save_to_data_dir<P: AsRef<Path>>(data_dir: P, records: Vec<NodeRecord>) {
        let path = data_dir.as_ref().join("server-list.json");
        NodeRecord::save_to_path(path, records)
    }

    pub fn save_to_path<P: AsRef<Path>>(path: P, records: Vec<NodeRecord>) {
        let json = serde_json::to_string_pretty(&records).unwrap();
        fs::write(path, json).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
    use tempfile::tempdir;

    use crate::protocols::defines::ProtocolCapability;

    #[test]
    fn load_from_path_file_not_exists_returns_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("not_exist.json");
        let records = NodeRecord::load_from_path(path);
        assert!(records.is_empty());
    }

    #[test]
    fn save_to_path_and_load_from_path_single_node() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("server-list.json");

        let protocols = ProtocolCapability::TCP;

        let node = NodeRecord {
            endpoint: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8000),
            protocols: protocols,
            first_seen: Utc.ymd(2025, 1, 1).and_hms(12, 0, 0),
            last_seen: Utc.ymd(2025, 1, 1).and_hms(12, 5, 0),
            last_disappeared: None,
            reachability_score: 1.0,
        };

        NodeRecord::save_to_path(&path, vec![node.clone()]);
        let loaded = NodeRecord::load_from_path(&path);
        assert_eq!(loaded.len(), 1);
        let loaded_node = &loaded[0];
        assert_eq!(loaded_node.endpoint, node.endpoint);
        assert_eq!(loaded_node.protocols, node.protocols);
        assert_eq!(loaded_node.first_seen, node.first_seen);
        assert_eq!(loaded_node.last_seen, node.last_seen);
        assert_eq!(loaded_node.last_disappeared, node.last_disappeared);
        assert_eq!(loaded_node.reachability_score, node.reachability_score);
    }

    #[test]
    fn save_to_path_and_load_from_path_multiple_nodes_ipv4_ipv6() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("server-list.json");

        let protocols = ProtocolCapability::TCP;

        let protocols1 = protocols | ProtocolCapability::UDP;

        let node1 = NodeRecord {
            endpoint: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 7000),
            protocols: protocols,
            first_seen: Utc.ymd(2025, 2, 1).and_hms(10, 0, 0),
            last_seen: Utc.ymd(2025, 2, 1).and_hms(10, 5, 0),
            last_disappeared: Some(Utc.ymd(2025, 2, 1).and_hms(11, 0, 0)),
            reachability_score: 0.9,
        };

        let mut protocols2 = ProtocolCapability::TCP;

        let node2 = NodeRecord {
            endpoint: SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 9000),
            protocols: protocols2,
            first_seen: Utc.ymd(2025, 2, 2).and_hms(12, 0, 0),
            last_seen: Utc.ymd(2025, 2, 2).and_hms(12, 10, 0),
            last_disappeared: None,
            reachability_score: 0.8,
        };

        NodeRecord::save_to_path(&path, vec![node1.clone(), node2.clone()]);
        let loaded = NodeRecord::load_from_path(&path);
        assert_eq!(loaded.len(), 2);

        // 验证 node1
        let l1 = loaded
            .iter()
            .find(|n| n.endpoint == node1.endpoint)
            .unwrap();
        assert_eq!(l1.protocols, node1.protocols);
        assert_eq!(l1.first_seen, node1.first_seen);
        assert_eq!(l1.last_disappeared, node1.last_disappeared);
        assert_eq!(l1.reachability_score, node1.reachability_score);

        // 验证 node2
        let l2 = loaded
            .iter()
            .find(|n| n.endpoint == node2.endpoint)
            .unwrap();
        assert!(l2.endpoint.is_ipv6());
        assert_eq!(l2.protocols, node2.protocols);
        assert_eq!(l2.reachability_score, node2.reachability_score);
    }

    #[test]
    fn save_to_data_dir_and_load_from_data_dir() {
        let dir = tempdir().unwrap();

        let protocols = ProtocolCapability::TCP;

        let node = NodeRecord {
            endpoint: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080),
            protocols: protocols,
            first_seen: Utc::now(),
            last_seen: Utc::now(),
            last_disappeared: None,
            reachability_score: 1.0,
        };

        NodeRecord::save_to_data_dir(dir.path(), vec![node.clone()]);
        let loaded = NodeRecord::load_from_data_dir(dir.path());
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].endpoint, node.endpoint);
    }
}
