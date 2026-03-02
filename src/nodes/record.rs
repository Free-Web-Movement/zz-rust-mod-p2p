use aex::connection::protocol::{ Protocol};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, SocketAddr},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeRecord {
    /// 使用 SocketAddr 作为唯一网络识别
    pub endpoint: SocketAddr,

    /// 该 endpoint 支持 / 曾成功使用过的协议集合
    pub protocols: HashSet<Protocol>,

    /// 首次发现时间
    pub first_seen: DateTime<Utc>,

    /// 最近一次成功通信
    pub last_seen: DateTime<Utc>,

    /// 是否已经连接成功， 用于内存状态标记，非持久化字段
    #[serde(skip)]
    pub connected: bool,
    #[serde(skip)]
    pub address: Option<String>,

    /// 最近一次确认不可达
    pub last_disappeared: Option<DateTime<Utc>>,

    /// 连通性评分（用于节点筛选 / 退避）
    pub reachability_score: u8,
}

impl NodeRecord {
    pub fn to_list(
        ip_addrs: Vec<IpAddr>,
        port: u16,
        protocols: HashSet<Protocol>,
    ) -> Vec<NodeRecord> {
        let now = Utc::now();
        ip_addrs
            .into_iter()
            .map(|ip| NodeRecord {
                endpoint: SocketAddr::new(ip, port),
                protocols: protocols.clone(),
                first_seen: now,
                last_seen: now,
                connected: false,
                last_disappeared: None,
                address: None,
                reachability_score: 100,
            })
            .collect()
    }

    /// 使用 dst 更新 self（self = older, dst = newer）
    pub fn update(mut self, other: NodeRecord) -> NodeRecord {
        debug_assert_eq!(self.endpoint, other.endpoint);
        // let _ = self.protocols.union(&other.protocols);
        for protocol in other.protocols {
            self.protocols.insert(protocol);
        }

        self.first_seen = self.first_seen.min(other.first_seen);
        self.last_seen = self.last_seen.max(other.last_seen);

        self.last_disappeared = match (self.last_disappeared, other.last_disappeared) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };

        self.reachability_score = self.reachability_score.max(other.reachability_score);

        self
    }

    pub fn merge(src: Vec<NodeRecord>, dst: Vec<NodeRecord>) -> Vec<NodeRecord> {
        let mut map: HashMap<SocketAddr, NodeRecord> = HashMap::new();

        // 先放 dst（本地已有）
        for record in dst {
            map.insert(record.endpoint, record);
        }

        // 再用 src 更新
        for record in src {
            map.entry(record.endpoint)
                .and_modify(|existing| {
                    let updated = existing.clone().update(record.clone());
                    *existing = updated;
                })
                .or_insert(record);
        }

        map.into_values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_to_list_single_ipv4() {
        let ip = IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8));
        let port = 3030;
        let mut protocols = HashSet::new();
        protocols.insert(Protocol::Tcp);
        protocols.insert(Protocol::Udp);

        let before = Utc::now();
        let list = NodeRecord::to_list(vec![ip], port, protocols.clone());
        let after = Utc::now();

        assert_eq!(list.len(), 1);

        let node = &list[0];

        assert_eq!(node.endpoint, SocketAddr::new(ip, port));
        assert_eq!(node.protocols, protocols);
        assert!(node.first_seen >= before && node.first_seen <= after);
        assert!(node.last_seen >= before && node.last_seen <= after);
        assert!(node.last_seen >= node.first_seen);
        assert!(node.last_disappeared.is_none());
        assert_eq!(node.reachability_score, 100);
    }

    #[test]
    fn test_to_list_ipv4_and_ipv6() {
        let ips = vec![
            IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
            IpAddr::V6("2001:4860:4860::8888".parse().unwrap()),
        ];

        let port = 4040;
        let mut protocols = HashSet::new();
        protocols.insert(Protocol::Tcp);

        let list = NodeRecord::to_list(ips.clone(), port, protocols.clone());

        assert_eq!(list.len(), 2);

        for (node, ip) in list.iter().zip(ips.iter()) {
            assert_eq!(node.endpoint, SocketAddr::new(*ip, port));
            assert_eq!(node.protocols, protocols);
            assert!(node.last_disappeared.is_none());
            assert_eq!(node.reachability_score, 100);
        }
    }

    #[test]
    fn test_to_list_empty_ips() {
        let list = NodeRecord::to_list(
            Vec::new(),
            1234,
            HashSet::new()
        );
        assert!(list.is_empty());
    }
}
