use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
};

use crate::protocols::defines::ProtocolCapability;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    pub reachability_score: u8,
}

impl NodeRecord {
    pub fn to_list(
        ip_addrs: Vec<IpAddr>,
        port: u16,
        protocol_capabilities: ProtocolCapability,
    ) -> Vec<NodeRecord> {
        let now = Utc::now();
        ip_addrs
            .into_iter()
            .map(|ip| NodeRecord {
                endpoint: SocketAddr::new(ip, port),
                protocols: protocol_capabilities,
                first_seen: now,
                last_seen: now,
                last_disappeared: None,
                reachability_score: 100,
            })
            .collect()
    }

    /// 使用 dst 更新 self（self = older, dst = newer）
    pub fn update(mut self, other: NodeRecord) -> NodeRecord {
        debug_assert_eq!(self.endpoint, other.endpoint);

        self.protocols |= other.protocols;

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
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn test_to_list_single_ipv4() {
        let ip = IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8));
        let port = 3030;
        let protocols = ProtocolCapability::TCP | ProtocolCapability::UDP;

        let before = Utc::now();
        let list = NodeRecord::to_list(vec![ip], port, protocols);
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
        let protocols = ProtocolCapability::TCP;

        let list = NodeRecord::to_list(ips.clone(), port, protocols);

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
            ProtocolCapability::TCP | ProtocolCapability::UDP,
        );
        assert!(list.is_empty());
    }

    #[test]
    fn test_protocol_capability_preserved() {
        let ip = IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9));
        let protocols =
            ProtocolCapability::TCP | ProtocolCapability::UDP | ProtocolCapability::WEBSOCKET;

        let list = NodeRecord::to_list(vec![ip], 8080, protocols);
        let node = &list[0];

        assert!(node.protocols.contains(ProtocolCapability::TCP));
        assert!(node.protocols.contains(ProtocolCapability::UDP));
        assert!(node.protocols.contains(ProtocolCapability::WEBSOCKET));
        assert!(!node.protocols.contains(ProtocolCapability::HTTP));
    }
}
